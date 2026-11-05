// Game test. Tells the plugin each frame whether the player is driving, because the wheel must
// only apply then. Four steering speed values are multiplied in memory when the game starts, for
// every vehicle drive model, by an amount the plugin supplies. Nothing is saved.
// During pause, it tests one way into a settings screen: a WHEEL SETTINGS entry in the pause menu,
// with the period key as a fallback. The plugin runs the screen, and this script passes it keys and
// draws a full page, laid out like the game's Key Bindings page. While that page is open the pause
// menu is hidden and its entries do nothing.

native func WheelDriveTest_Ping(value: Int32) -> Int32;
native func WheelDriveTest_Event(name: String);
native func WheelDriveTest_Read() -> Int32;
native func WheelDriveTest_Key() -> Int32;
native func WheelDriveTest_SetDriving(driving: Bool);
native func WheelDriveTest_SteeringAction(value: Float);
native func WheelDriveTest_SteeringSpeed() -> Float;
native func WheelDriveTest_ScreenOpen();
native func WheelDriveTest_ScreenClose();
native func WheelDriveTest_ScreenCommand(code: Int32);
native func WheelDriveTest_ScreenTick() -> Bool;
native func WheelDriveTest_ScreenRowCount() -> Int32;
native func WheelDriveTest_ScreenRowLabel(row: Int32) -> String;
native func WheelDriveTest_ScreenRowValue(row: Int32) -> String;
native func WheelDriveTest_ScreenSelected() -> Int32;
native func WheelDriveTest_ScreenSelect(row: Int32);
native func WheelDriveTest_ScreenStatus() -> String;
native func WheelDriveTest_ScreenHints() -> String;

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
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"WheelDriveTestService") as WheelDriveTestService;
    if IsDefined(service) {
        service.SetMenuRoot(this.GetRootWidget());
    }
}

@wrapMethod(PauseMenuGameController)
protected cb func OnMenuItemActivated(index: Int32, target: ref<ListItemController>) -> Bool {
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"WheelDriveTestService") as WheelDriveTestService;
    if IsDefined(service) && service.IsScreenOpen() {
        return true;
    }
    let data = target.GetData() as PauseMenuListItemData;
    if IsDefined(data) && Equals(data.label, WheelDriveTestService.EntryLabel()) {
        WheelDriveTest_Event("wheel-entry-activated");
        if IsDefined(service) {
            service.OpenScreen();
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
    private let m_menuRoot: wref<inkWidget>;
    private let m_rowValues: array<ref<inkText>>;
    private let m_rowMarks: array<ref<inkRectangle>>;
    private let m_status: ref<inkText>;
    private let m_hints: ref<inkText>;
    private let m_listedMenuFunctions: Bool;

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
        if !this.m_paused {
            return;
        }
        if !this.IsScreenOpen() {
            if Equals(event.GetKey(), EInputKey.IK_Period) {
                WheelDriveTest_Event("wheel-key-activated");
                this.OpenScreen();
            }
            return;
        }
        switch event.GetKey() {
            case EInputKey.IK_Up:
                WheelDriveTest_ScreenCommand(1);
                break;
            case EInputKey.IK_Down:
                WheelDriveTest_ScreenCommand(2);
                break;
            case EInputKey.IK_Enter:
                WheelDriveTest_ScreenCommand(3);
                break;
            case EInputKey.IK_Delete:
                WheelDriveTest_ScreenCommand(4);
                break;
            case EInputKey.IK_Period:
                WheelDriveTest_ScreenCommand(5);
                break;
            default:
                break;
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
        hint.SetText("");
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

    public func IsScreenOpen() -> Bool {
        return IsDefined(this.m_screen);
    }

    public func SetMenuRoot(root: ref<inkWidget>) {
        this.m_menuRoot = root;
    }

    private func Red() -> HDRColor {
        return new HDRColor(1.1761, 0.3809, 0.3476, 1.0);
    }

    private func Teal() -> HDRColor {
        return new HDRColor(0.3686, 0.9647, 1.0, 1.0);
    }

    private func AddRect(parent: ref<inkCanvas>, x: Float, y: Float, w: Float, h: Float, color: HDRColor) -> ref<inkRectangle> {
        let rect = new inkRectangle();
        rect.SetAnchor(inkEAnchor.TopLeft);
        rect.SetMargin(new inkMargin(x, y, 0.0, 0.0));
        rect.SetSize(new Vector2(w, h));
        rect.SetTintColor(color);
        rect.Reparent(parent);
        return rect;
    }

    private func AddText(parent: ref<inkCanvas>, font: String, size: Int32, x: Float, y: Float, w: Float, h: Float, color: HDRColor, centered: Bool) -> ref<inkText> {
        let text = new inkText();
        text.SetFontFamily(font);
        text.SetFontSize(size);
        text.SetTintColor(color);
        text.SetAnchor(inkEAnchor.TopLeft);
        text.SetMargin(new inkMargin(x, y, 0.0, 0.0));
        text.SetSize(new Vector2(w, h));
        text.SetHorizontalAlignment(centered ? textHorizontalAlignment.Center : textHorizontalAlignment.Left);
        text.SetVerticalAlignment(textVerticalAlignment.Center);
        text.Reparent(parent);
        return text;
    }

    public func OpenScreen() {
        if IsDefined(this.m_screen) {
            return;
        }
        let layer = this.MenuLayer();
        if !IsDefined(layer) {
            WheelDriveTest_Event("menu-layer-missing");
            return;
        }
        WheelDriveTest_ScreenOpen();
        this.ListMenuFunctions();

        let body = "base\\gameplay\\gui\\fonts\\raj\\raj.inkfontfamily";
        let heading = "base\\gameplay\\gui\\fonts\\orbitron\\orbitron.inkfontfamily";

        let page = new inkCanvas();
        page.SetAnchor(inkEAnchor.Fill);
        page.Reparent(layer.GetVirtualWindow());

        let back = new inkRectangle();
        back.SetAnchor(inkEAnchor.Fill);
        back.SetTintColor(new HDRColor(0.03, 0.01, 0.02, 1.0));
        back.Reparent(page);

        let content = new inkCanvas();
        content.SetAnchor(inkEAnchor.TopCenter);
        content.SetAnchorPoint(0.5, 0.0);
        content.SetSize(new Vector2(2000.0, 1400.0));
        content.SetMargin(new inkMargin(0.0, 60.0, 0.0, 0.0));
        content.Reparent(page);

        this.AddText(content, heading, 56, 0.0, 0.0, 2000.0, 90.0, this.Teal(), true).SetText("WHEEL SETTINGS");
        this.AddRect(content, 0.0, 130.0, 2000.0, 3.0, this.Teal());
        this.AddText(content, body, 42, 20.0, 140.0, 1000.0, 70.0, new HDRColor(0.85, 0.85, 0.85, 1.0), false).SetText("Driving");

        ArrayClear(this.m_rowValues);
        ArrayClear(this.m_rowMarks);
        let count = WheelDriveTest_ScreenRowCount();
        let row = 0;
        while row < count {
            let y = 220.0 + Cast<Float>(row) * 100.0;
            let mark = this.AddRect(content, 0.0, y, 2000.0, 90.0, this.Red());
            mark.SetOpacity(0.03);
            mark.SetName(StringToName(IntToString(row)));
            mark.SetInteractive(true);
            mark.RegisterToCallback(n"OnRelease", this, n"OnRowRelease");
            mark.RegisterToCallback(n"OnHoverOver", this, n"OnRowHover");
            ArrayPush(this.m_rowMarks, mark);

            this.AddText(content, body, 42, 30.0, y, 1000.0, 90.0, this.Red(), false).SetText(WheelDriveTest_ScreenRowLabel(row));

            let boxX = 1050.0;
            let boxW = 900.0;
            this.AddRect(content, boxX, y + 8.0, boxW, 2.0, this.Red());
            this.AddRect(content, boxX, y + 80.0, boxW, 2.0, this.Red());
            this.AddRect(content, boxX, y + 8.0, 2.0, 74.0, this.Red());
            this.AddRect(content, boxX + boxW - 2.0, y + 8.0, 2.0, 74.0, this.Red());
            ArrayPush(this.m_rowValues, this.AddText(content, body, 40, boxX, y, boxW, 90.0, this.Teal(), true));
            row += 1;
        }

        let bottom = 220.0 + Cast<Float>(count) * 100.0;
        this.m_status = this.AddText(content, heading, 34, 0.0, bottom + 20.0, 2000.0, 70.0, this.Teal(), true);
        this.m_hints = this.AddText(content, heading, 30, 0.0, bottom + 100.0, 2000.0, 60.0, this.Red(), true);

        this.m_screen = page;
        if IsDefined(this.m_menuRoot) {
            this.m_menuRoot.SetVisible(false);
        }
        this.UpdateScreen();
        WheelDriveTest_Event("wheel-screen-open");
    }

    // Logged once, to help find how the pause menu handles going back.
    private func ListMenuFunctions() {
        if this.m_listedMenuFunctions {
            return;
        }
        this.m_listedMenuFunctions = true;
        let cls = Reflection.GetClass(n"PauseMenuGameController");
        while IsDefined(cls) {
            for func in cls.GetFunctions() {
                WheelDriveTest_Event(s"menu-function: \(NameToString(cls.GetName())).\(NameToString(func.GetName()))");
            }
            cls = cls.GetParent();
        }
    }

    protected cb func OnRowHover(event: ref<inkPointerEvent>) -> Bool {
        WheelDriveTest_ScreenSelect(StringToInt(NameToString(event.GetCurrentTarget().GetName())));
    }

    protected cb func OnRowRelease(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            WheelDriveTest_Event("wheel-row-clicked");
            WheelDriveTest_ScreenSelect(StringToInt(NameToString(event.GetCurrentTarget().GetName())));
            WheelDriveTest_ScreenCommand(3);
        }
    }

    private func HideScreen() {
        if IsDefined(this.m_screen) {
            WheelDriveTest_ScreenClose();
            let layer = this.MenuLayer();
            if IsDefined(layer) {
                layer.GetVirtualWindow().RemoveChild(this.m_screen);
            } else {
                this.m_screen.SetVisible(false);
            }
            this.m_screen = null;
            this.m_status = null;
            this.m_hints = null;
            ArrayClear(this.m_rowValues);
            ArrayClear(this.m_rowMarks);
            if IsDefined(this.m_menuRoot) {
                this.m_menuRoot.SetVisible(true);
            }
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
        if !IsDefined(this.m_screen) {
            return;
        }
        if !WheelDriveTest_ScreenTick() {
            this.HideScreen();
            return;
        }
        let selected = WheelDriveTest_ScreenSelected();
        let row = 0;
        while row < ArraySize(this.m_rowValues) {
            this.m_rowValues[row].SetText(WheelDriveTest_ScreenRowValue(row));
            this.m_rowMarks[row].SetOpacity(row == selected ? 0.22 : 0.03);
            row += 1;
        }
        this.m_status.SetText(WheelDriveTest_ScreenStatus());
        this.m_hints.SetText(WheelDriveTest_ScreenHints());
    }
}
