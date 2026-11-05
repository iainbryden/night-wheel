// Game test. Tells the plugin each frame whether the player is driving, because the wheel must
// only apply then. Four steering speed values are multiplied in memory when the game starts, for
// every vehicle drive model, by an amount the plugin supplies. Nothing is saved.
// During pause, it tests one way into a settings screen: a WHEEL SETTINGS entry in the pause menu,
// with the period key as a fallback. The screen only shows live values and has no settings yet.

native func WheelDriveTest_Ping(value: Int32) -> Int32;
native func WheelDriveTest_Event(name: String);
native func WheelDriveTest_Read() -> Int32;
native func WheelDriveTest_Key() -> Int32;
native func WheelDriveTest_SetDriving(driving: Bool);
native func WheelDriveTest_SteeringAction(value: Float);
native func WheelDriveTest_SteeringSpeed() -> Float;
native func WheelDriveTest_HeldControls() -> String;

public class WheelDriveStartupRates extends ScriptableTweak {
    protected cb func OnApply() -> Void {
        let multiplier = WheelDriveTest_SteeringSpeed();
        let names: array<String>;
        ArrayPush(names, ".turnUpdateInputFastChangeSpeed");
        ArrayPush(names, ".turnUpdateInputSlowChangeSpeed");
        ArrayPush(names, ".wheelTurnMaxAddPerSecond");
        ArrayPush(names, ".wheelTurnMaxSubPerSecond");

        let records = TweakDBInterface.GetRecords(n"VehicleDriveModelData");
        let changed = 0;
        for record in records {
            let base = record.GetID();
            for name in names {
                let id = base;
                TDBID.Append(id, TDBID.Create(name));
                let value = TweakDBInterface.GetFloat(id, -1.0);
                if value > 0.0 && TweakDBManager.SetFlat(id, value * multiplier) {
                    changed += 1;
                }
            }
            TweakDBManager.UpdateRecord(base);
        }
        WheelDriveTest_Event(s"startup rates: \(ArraySize(records)) drive models, \(changed) values multiplied by \(FloatToString(multiplier))");
    }
}

// Adds the WHEEL SETTINGS entry after the game's own entries, and leaves those entries to the game.
@wrapMethod(PauseMenuGameController)
private func PopulateMenuItemList() -> Void {
    wrappedMethod();
    this.AddMenuItem(WheelDriveTestService.EntryLabel(), n"OnWheelDriveOpen");
    this.m_menuListController.Refresh();
    WheelDriveTest_Event("wheel-entry-added");
}

@wrapMethod(PauseMenuGameController)
protected cb func OnMenuItemActivated(index: Int32, target: ref<ListItemController>) -> Bool {
    let data = target.GetData() as PauseMenuListItemData;
    if IsDefined(data) && Equals(data.label, WheelDriveTestService.EntryLabel()) {
        WheelDriveTest_Event("wheel-entry-activated");
        let service = GameInstance.GetScriptableServiceContainer().GetService(n"WheelDriveTestService") as WheelDriveTestService;
        if IsDefined(service) {
            service.ToggleScreen();
        } else {
            WheelDriveTest_Event("wheel-service-missing");
        }
        return true;
    }
    return wrappedMethod(index, target);
}

public class WheelDriveTestTick extends DelayCallback {
    public let service: wref<WheelDriveTestService>;

    public func Call() {
        if IsDefined(this.service) {
            this.service.OnTick();
        }
    }
}

public class WheelDriveTestService extends ScriptableService {
    private let m_active: Bool;
    private let m_driving: Bool;
    private let m_listening: wref<PlayerPuppet>;
    private let m_paused: Bool;
    private let m_steering: Int32;
    private let m_hint: ref<inkText>;
    private let m_refresh: ref<inkAnimProxy>;
    private let m_screen: ref<inkCanvas>;
    private let m_screenText: ref<inkText>;
    private let m_held: String;

    public static func EntryLabel() -> String {
        return "WHEEL SETTINGS";
    }

    private cb func OnLoad() {
        WheelDriveTest_Event("script-loaded");
        WheelDriveTest_Ping(41);

        let callbacks = GameInstance.GetCallbackSystem();
        callbacks.RegisterCallback(n"Session/Ready", this, n"OnSessionReady");
        callbacks.RegisterCallback(n"Session/Pause", this, n"OnSessionPause");
        callbacks.RegisterCallback(n"Session/Resume", this, n"OnSessionResume");
        callbacks.RegisterCallback(n"Session/End", this, n"OnSessionEnd");
        callbacks.RegisterCallback(n"Input/Key", this, n"OnKey");
    }

    private cb func OnSessionReady(event: ref<GameSessionEvent>) {
        WheelDriveTest_Event("session-ready");
        if !this.m_active {
            this.m_active = true;
            this.ScheduleTick();
        }
    }

    private cb func OnSessionPause(event: ref<GameSessionEvent>) {
        this.m_paused = true;
        WheelDriveTest_Event("session-pause");
        this.ShowHint();
    }

    private cb func OnSessionResume(event: ref<GameSessionEvent>) {
        this.m_paused = false;
        WheelDriveTest_Event("session-resume");
        this.HideAll();
    }

    private cb func OnSessionEnd(event: ref<GameSessionEvent>) {
        this.m_active = false;
        this.m_paused = false;
        this.m_driving = false;
        this.m_listening = null;
        WheelDriveTest_SetDriving(false);
        WheelDriveTest_Event("session-end");
        this.HideAll();
    }

    // Records that a press arrived, and never which key outside the one test key.
    private cb func OnKey(event: ref<KeyInputEvent>) {
        if NotEquals(event.GetAction(), EInputAction.IACT_Press) {
            return;
        }
        WheelDriveTest_Key();
        if this.m_paused && Equals(event.GetKey(), EInputKey.IK_Period) {
            WheelDriveTest_Event("wheel-key-activated");
            this.ToggleScreen();
        }
    }

    public func OnTick() {
        if !this.m_active {
            return;
        }
        this.UpdateDriving();
        this.ScheduleTick();
    }

    private func UpdateDriving() {
        let game = GetGameInstance();
        let player = GetPlayer(game);
        let driving = IsDefined(player) && VehicleComponent.IsDriver(game, player);
        WheelDriveTest_SetDriving(driving);
        if NotEquals(driving, this.m_driving) {
            this.m_driving = driving;
            WheelDriveTest_Event(driving ? "driving-on" : "driving-off");
            if driving {
                this.StartListening(player);
                this.ReportRates(player);
            }
        }
    }

    private func StartListening(player: ref<PlayerPuppet>) {
        if !IsDefined(this.m_listening) {
            player.RegisterInputListener(this, n"TurnX");
            this.m_listening = player;
            WheelDriveTest_Event("listening-for-steering-action");
        }
    }

    protected cb func OnAction(action: ListenerAction, consumer: ListenerActionConsumer) -> Bool {
        if Equals(ListenerAction.GetName(action), n"TurnX") {
            WheelDriveTest_SteeringAction(ListenerAction.GetValue(action));
        }
    }

    // Reads the values through the record, which shows whether the change reached it.
    private func ReportRates(player: ref<PlayerPuppet>) {
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) {
            return;
        }
        let model = vehicle.GetRecord().VehDriveModelDataHandle();
        if IsDefined(model) {
            WheelDriveTest_Event(s"rate: entity \(EntityID.ToDebugString(vehicle.GetEntityID())) fast \(FloatToString(model.TurnUpdateInputFastChangeSpeed())) slow \(FloatToString(model.TurnUpdateInputSlowChangeSpeed())) add \(FloatToString(model.WheelTurnMaxAddPerSecond())) sub \(FloatToString(model.WheelTurnMaxSubPerSecond()))");
        }
    }

    private func ScheduleTick() {
        let tick = new WheelDriveTestTick();
        tick.service = this;
        GameInstance.GetDelaySystem(GetGameInstance()).DelayCallback(tick, 0.0, false);
    }

    private func MenuLayer() -> ref<inkLayerWrapper> {
        return GameInstance.GetInkSystem().GetLayer(n"inkMenuLayer");
    }

    private func NewText(size: Int32) -> ref<inkText> {
        let text = new inkText();
        text.SetFontFamily("base\\gameplay\\gui\\fonts\\orbitron\\orbitron.inkfontfamily");
        text.SetFontStyle(n"Bold");
        text.SetFontSize(size);
        text.SetTintColor(new HDRColor(1.1761, 0.3809, 0.3476, 1.0));
        return text;
    }

    private func ShowHint() {
        if IsDefined(this.m_hint) {
            return;
        }
        let layer = this.MenuLayer();
        if !IsDefined(layer) {
            WheelDriveTest_Event("menu-layer-missing");
            return;
        }
        let hint = this.NewText(32);
        hint.SetAnchor(inkEAnchor.TopCenter);
        hint.SetAnchorPoint(0.5, 0.0);
        hint.SetMargin(new inkMargin(0.0, 40.0, 0.0, 0.0));
        hint.SetText("NIGHT WHEEL TEST  PERIOD ALSO OPENS WHEEL SETTINGS");
        hint.Reparent(layer.GetVirtualWindow());
        this.m_hint = hint;
        this.StartRefresh();
        WheelDriveTest_Event("text-shown");
    }

    // The delay system mostly stops during pause, so a looping UI animation drives the display.
    // It leaves the widget fully visible and only supplies the update.
    private func StartRefresh() {
        let hold = new inkAnimTransparency();
        hold.SetStartTransparency(1.0);
        hold.SetEndTransparency(1.0);
        hold.SetDuration(0.016);

        let definition = new inkAnimDef();
        definition.AddInterpolator(hold);

        let options: inkAnimOptions;
        options.loopType = inkanimLoopType.Cycle;
        options.loopInfinite = true;

        this.m_refresh = this.m_hint.PlayAnimationWithOptions(definition, options);
        this.m_refresh.RegisterToCallback(inkanimEventType.OnEndLoop, this, n"OnRefresh");
    }

    private cb func OnRefresh(proxy: ref<inkAnimProxy>) -> Bool {
        this.m_steering = WheelDriveTest_Read();
        this.UpdateScreen();
    }

    public func ToggleScreen() {
        if IsDefined(this.m_screen) {
            this.HideScreen();
        } else {
            this.ShowScreen();
        }
    }

    private func ShowScreen() {
        let layer = this.MenuLayer();
        if !IsDefined(layer) {
            WheelDriveTest_Event("menu-layer-missing");
            return;
        }
        let canvas = new inkCanvas();
        canvas.SetAnchor(inkEAnchor.Fill);
        canvas.Reparent(layer.GetVirtualWindow());

        let back = new inkRectangle();
        back.SetAnchor(inkEAnchor.Centered);
        back.SetAnchorPoint(0.5, 0.5);
        back.SetSize(new Vector2(1500.0, 520.0));
        back.SetTintColor(new HDRColor(0.03, 0.03, 0.05, 1.0));
        back.Reparent(canvas);

        let text = this.NewText(44);
        text.SetAnchor(inkEAnchor.Centered);
        text.SetAnchorPoint(0.5, 0.5);
        text.Reparent(canvas);

        this.m_screen = canvas;
        this.m_screenText = text;
        this.UpdateScreen();
        WheelDriveTest_Event("wheel-screen-open");
    }

    private func HideScreen() {
        if IsDefined(this.m_screen) {
            let layer = this.MenuLayer();
            if IsDefined(layer) {
                layer.GetVirtualWindow().RemoveChild(this.m_screen);
            } else {
                this.m_screen.SetVisible(false);
            }
            this.m_screen = null;
            this.m_screenText = null;
            WheelDriveTest_Event("wheel-screen-closed");
        }
    }

    private func HideAll() {
        this.HideScreen();
        if IsDefined(this.m_refresh) {
            this.m_refresh.Stop();
            this.m_refresh = null;
        }
        if IsDefined(this.m_hint) {
            this.m_hint.SetVisible(false);
            this.m_hint = null;
        }
    }

    private func UpdateScreen() {
        if IsDefined(this.m_screenText) {
            let held = WheelDriveTest_HeldControls();
            if NotEquals(held, this.m_held) {
                this.m_held = held;
                WheelDriveTest_Event(s"held: \(held)");
            }
            this.m_screenText.SetText(s"NIGHT WHEEL\nSTEERING VALUE \(this.m_steering)\nHOLDING \(held)\nSTEERING SPEED X\(FloatToString(WheelDriveTest_SteeringSpeed()))\nPERIOD CLOSES THIS");
        }
    }
}
