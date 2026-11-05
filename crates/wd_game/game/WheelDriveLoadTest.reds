// Game test. Tells the plugin each frame whether the player is driving, because the wheel must
// only apply then. Four steering speed values are multiplied in memory when the game starts, for
// every vehicle drive model, by an amount the plugin supplies. Nothing is saved.
// During pause, it tests one way into a settings screen: a WHEEL SETTINGS entry in the pause menu,
// with the period key as a fallback. The plugin runs the screen, and this script passes it keys and
// draws a full page, laid out like the game's Key Bindings page. While that page is open the pause
// menu is hidden and its entries do nothing, and going back returns to the pause menu.

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
native func WheelDriveTest_ScreenRowLit(row: Int32) -> Bool;
native func WheelDriveTest_ScreenSection(row: Int32) -> String;
native func WheelDriveTest_TakeScriptActions() -> Int32;
native func WheelDriveTest_ScreenColumn(row: Int32) -> Int32;
native func WheelDriveTest_ScreenBoostRow() -> Int32;
native func WheelDriveTest_BoostPercent() -> Int32;
native func WheelDriveTest_BoostAdjust(steps: Int32);
native func WheelDriveTest_BoostWanted() -> Bool;
native func WheelDriveTest_SetAutopilot(on: Bool);
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

// Going back cancels a bind or leaves the page, in place of closing the pause menu.
@wrapMethod(PauseMenuGameController)
protected cb func OnGlobalRelease(e: ref<inkPointerEvent>) -> Bool {
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"WheelDriveTestService") as WheelDriveTestService;
    if IsDefined(service) && service.IsScreenOpen() && e.IsAction(n"back") {
        WheelDriveTest_Event("wheel-back");
        WheelDriveTest_ScreenCommand(5);
        return true;
    }
    return wrappedMethod(e);
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
    private let m_inMenu: Bool;
    private let m_lastBoostEnd: Float;
    private let m_autopilot: Bool;
    private let m_boosting: Bool;
    private let m_boostPeak: Float;
    private let m_boostLimit: Float;
    private let m_topSeen: Float;
    private let m_boostVehicle: EntityID;
    private let m_lastBoostTime: Float;
    private let m_reportedEngine: Bool;
    private let m_boostValue: ref<inkText>;
    private let m_boostFill: ref<inkRectangle>;

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
            case EInputKey.IK_Left:
                WheelDriveTest_BoostAdjust(-1);
                break;
            case EInputKey.IK_Right:
                WheelDriveTest_BoostAdjust(1);
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
        let inMenu = this.IsInMenu();
        if NotEquals(inMenu, this.m_inMenu) {
            this.m_inMenu = inMenu;
            WheelDriveTest_Event(s"in-menu: \(inMenu)");
        }
        let driving = !inMenu && IsDefined(player) && VehicleComponent.IsDriver(game, player);
        WheelDriveTest_SetDriving(driving);
        if driving {
            let actions = WheelDriveTest_TakeScriptActions();
            if actions != 0 {
                this.RunRadioActions(player, actions);
            }
            this.UpdateAutopilot(player);
            this.UpdateBoost(player);
        } else {
            this.EndBoost("not driving");
            if this.m_autopilot {
                this.m_autopilot = false;
                WheelDriveTest_SetAutopilot(false);
            }
        }
        if NotEquals(driving, this.m_driving) {
            this.m_driving = driving;
            WheelDriveTest_Event(driving ? "driving-on" : "driving-off");
            if driving {
                this.StartListening(player);
                this.ReportRates(player);
            }
        }
    }

    // The game's own driving must not have the wheel steering against it. The flag's name comes
    // from the game's vehicle blackboard, and the log shows whether it follows Autodrive.
    private func UpdateAutopilot(player: ref<PlayerPuppet>) {
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) {
            return;
        }
        let board = vehicle.GetBlackboard();
        if !IsDefined(board) {
            return;
        }
        let on = board.GetBool(GetAllBlackboardDefs().Vehicle.IsAutopilotOn);
        if NotEquals(on, this.m_autopilot) {
            this.m_autopilot = on;
            WheelDriveTest_SetAutopilot(on);
            WheelDriveTest_Event(s"autopilot: \(on)");
        }
    }

    // Boost pushes the car forward through the physics system while the control is held, up to a
    // limit above the fastest speed seen without boost. It changes no vehicle data and saves
    // nothing. The push strength is a provisional test value.
    private func UpdateBoost(player: ref<PlayerPuppet>) {
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) {
            return;
        }
        this.ReportEngine(vehicle);
        let id = vehicle.GetEntityID();
        if NotEquals(id, this.m_boostVehicle) {
            this.m_boostVehicle = id;
            this.m_topSeen = 0.0;
            this.EndBoost("new vehicle");
        }
        let speed = vehicle.GetCurrentSpeed();
        let now = EngineTime.ToFloat(GameInstance.GetSimTime(GetGameInstance()));
        let elapsed = now - this.m_lastBoostTime;
        this.m_lastBoostTime = now;
        if !WheelDriveTest_BoostWanted() || this.m_autopilot {
            this.EndBoost("released");
            if speed > this.m_topSeen && now - this.m_lastBoostEnd > 15.0 {
                this.m_topSeen = speed;
            }
            return;
        }
        let percent = Cast<Float>(WheelDriveTest_BoostPercent());
        if !this.m_boosting {
            this.m_boosting = true;
            this.m_boostPeak = speed;
            this.m_boostLimit = MaxF(this.m_topSeen, speed) * (1.0 + percent / 100.0);
            WheelDriveTest_Event(s"boost: on at \(FloatToString(speed)), limit \(FloatToString(this.m_boostLimit)), fastest seen \(FloatToString(this.m_topSeen)), strength \(FloatToString(percent))");
        }
        if speed > this.m_boostPeak {
            this.m_boostPeak = speed;
        }
        this.m_lastBoostEnd = now;
        if speed < 1.0 || speed >= this.m_boostLimit || elapsed <= 0.0 || elapsed > 0.1 {
            return;
        }
        let acceleration = MinF(6.0 + percent * 0.12, 30.0);
        let push = vehicle.GetTotalMass() * acceleration * elapsed;
        let forward = vehicle.GetWorldForward();
        let event = new PhysicalImpulseEvent();
        event.radius = 1.0;
        event.worldPosition = Vector4.Vector4To3(vehicle.GetWorldPosition());
        event.worldImpulse = new Vector3(forward.X * push, forward.Y * push, forward.Z * push);
        vehicle.QueueEvent(event);
    }

    private func EndBoost(reason: String) {
        if this.m_boosting {
            this.m_boosting = false;
            WheelDriveTest_Event(s"boost: off, \(reason), peak \(FloatToString(this.m_boostPeak))");
        }
    }

    // Logged once, to look for a value for the car's real top speed.
    private func ReportEngine(vehicle: ref<VehicleObject>) {
        if this.m_reportedEngine {
            return;
        }
        this.m_reportedEngine = true;
        let record = vehicle.GetRecord().VehEngineDataHandle();
        if !IsDefined(record) {
            WheelDriveTest_Event("engine: no record");
            return;
        }
        let cls = Reflection.GetClassOf(ToVariant(record), true);
        for func in cls.GetFunctions() {
            let type = func.GetReturnType();
            let parameters = func.GetParameters();
            if IsDefined(type) && ArraySize(parameters) == 0 && Equals(type.GetName(), n"Float") {
                WheelDriveTest_Event(s"engine: \(NameToString(func.GetName())) = \(FloatToString(FromVariant<Float>(func.Call(record))))");
            }
        }
    }

    // Radio functions with no gamepad button. Each logs what it did, because these are game
    // functions proven only by compiling against the game's scripts.
    private func RunRadioActions(player: ref<PlayerPuppet>, actions: Int32) {
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) {
            return;
        }
        if (actions & 1) != 0 {
            let on = vehicle.IsRadioReceiverActive();
            vehicle.ToggleRadioReceiver(!on);
            WheelDriveTest_Event(s"radio: was on=\(on), toggled");
        }
        if (actions & 2) != 0 {
            vehicle.NextRadioReceiverStation();
            WheelDriveTest_Event(s"radio: next, index \(vehicle.GetCurrentRadioIndex())");
        }
        if (actions & 4) != 0 {
            let index = Cast<Int32>(vehicle.GetCurrentRadioIndex());
            let position = RadioStationDataProvider.GetRadioStationUIIndex(index);
            if position > 0 {
                let previous = EnumInt(RadioStationDataProvider.GetRadioStationByUIIndex(position - 1));
                vehicle.SetRadioReceiverStation(Cast<Uint32>(previous));
            }
            WheelDriveTest_Event(s"radio: previous from position \(position), index \(vehicle.GetCurrentRadioIndex())");
        }
        if (actions & 8) != 0 {
            this.ChangeRadioVolume(1);
        }
        if (actions & 16) != 0 {
            this.ChangeRadioVolume(-1);
        }
    }

    private func ChangeRadioVolume(direction: Int32) {
        let settings = GameInstance.GetSettingsSystem(GetGameInstance());
        let volume = settings.GetVar(n"/audio/volume", n"CarRadioVolume") as ConfigVarInt;
        if !IsDefined(volume) {
            WheelDriveTest_Event("radio: volume setting not found");
            return;
        }
        let value = volume.GetValue() + direction * volume.GetStepValue();
        value = Max(volume.GetMinValue(), Min(volume.GetMaxValue(), value));
        volume.SetValue(value);
        if settings.NeedsConfirmation() {
            settings.ConfirmChanges();
        }
        WheelDriveTest_Event(s"radio: volume \(value)");
    }

    // In a menu such as the map, the wheel must stay out of the way, or its steering makes the game
    // switch to gamepad controls.
    private func IsInMenu() -> Bool {
        let defs = GetAllBlackboardDefs().UI_System;
        let board = GameInstance.GetBlackboardSystem(GetGameInstance()).Get(defs);
        return IsDefined(board) && board.GetBool(defs.IsInMenu);
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
        content.SetSize(new Vector2(2640.0, 1400.0));
        content.SetMargin(new inkMargin(0.0, 50.0, 0.0, 0.0));
        content.Reparent(page);

        this.AddText(content, heading, 56, 0.0, 0.0, 2640.0, 90.0, this.Teal(), true).SetText("WHEEL SETTINGS");

        ArrayClear(this.m_rowValues);
        ArrayClear(this.m_rowMarks);
        let count = WheelDriveTest_ScreenRowCount();
        let boostRow = WheelDriveTest_ScreenBoostRow();
        let columnY: array<Float>;
        ArrayPush(columnY, 100.0);
        ArrayPush(columnY, 100.0);
        let columnW = 1280.0;
        let row = 0;
        while row < count {
            let column = WheelDriveTest_ScreenColumn(row);
            let x = Cast<Float>(column) * (columnW + 80.0);
            let y = columnY[column];
            let section = WheelDriveTest_ScreenSection(row);
            if StrLen(section) > 0 {
                y += 16.0;
                let band = this.AddRect(content, x, y, columnW, 62.0, new HDRColor(0.05, 0.08, 0.11, 1.0));
                band.SetOpacity(0.9);
                this.AddRect(content, x, y, columnW, 3.0, this.Teal());
                this.AddText(content, body, 38, x + 20.0, y, columnW - 40.0, 62.0, new HDRColor(0.9, 0.9, 0.9, 1.0), false).SetText(section);
                y += 70.0;
            }
            let mark = this.AddRect(content, x, y, columnW, 68.0, this.Red());
            mark.SetOpacity(0.03);
            mark.SetName(StringToName(IntToString(row)));
            mark.SetInteractive(true);
            mark.RegisterToCallback(n"OnRelease", this, n"OnRowRelease");
            mark.RegisterToCallback(n"OnHoverOver", this, n"OnRowHover");
            ArrayPush(this.m_rowMarks, mark);

            this.AddText(content, body, 36, x + 20.0, y, 620.0, 68.0, this.Red(), false).SetText(WheelDriveTest_ScreenRowLabel(row));

            let boxX = x + 640.0;
            let boxW = 620.0;
            this.AddRect(content, boxX, y + 6.0, boxW, 2.0, this.Red());
            this.AddRect(content, boxX, y + 60.0, boxW, 2.0, this.Red());
            this.AddRect(content, boxX, y + 6.0, 2.0, 56.0, this.Red());
            this.AddRect(content, boxX + boxW - 2.0, y + 6.0, 2.0, 56.0, this.Red());
            ArrayPush(this.m_rowValues, this.AddText(content, body, 34, boxX, y, boxW, 68.0, this.Teal(), true));
            y += 72.0;

            if row == boostRow {
                this.AddText(content, body, 36, x + 20.0, y, 620.0, 68.0, this.Red(), false).SetText("Boost strength");
                let down = this.AddRect(content, boxX, y + 6.0, 70.0, 56.0, this.Red());
                down.SetOpacity(0.35);
                down.SetInteractive(true);
                down.RegisterToCallback(n"OnRelease", this, n"OnBoostDown");
                this.AddText(content, body, 40, boxX, y, 70.0, 68.0, new HDRColor(1.0, 1.0, 1.0, 1.0), true).SetText("-");
                let up = this.AddRect(content, boxX + boxW - 70.0, y + 6.0, 70.0, 56.0, this.Red());
                up.SetOpacity(0.35);
                up.SetInteractive(true);
                up.RegisterToCallback(n"OnRelease", this, n"OnBoostUp");
                this.AddText(content, body, 40, boxX + boxW - 70.0, y, 70.0, 68.0, new HDRColor(1.0, 1.0, 1.0, 1.0), true).SetText("+");
                let trackX = boxX + 80.0;
                let trackW = boxW - 160.0;
                let track = this.AddRect(content, trackX, y + 6.0, trackW, 56.0, this.Red());
                track.SetOpacity(0.08);
                this.m_boostFill = this.AddRect(content, trackX, y + 6.0, trackW, 56.0, this.Teal());
                this.m_boostFill.SetOpacity(0.45);
                this.m_boostValue = this.AddText(content, body, 34, trackX, y, trackW, 68.0, new HDRColor(1.0, 1.0, 1.0, 1.0), true);
                y += 72.0;
            }
            columnY[column] = y;
            row += 1;
        }

        let bottom = MaxF(columnY[0], columnY[1]);
        this.m_status = this.AddText(content, heading, 30, 0.0, bottom + 8.0, 2640.0, 50.0, this.Teal(), true);
        this.m_hints = this.AddText(content, heading, 26, 0.0, bottom + 62.0, 2640.0, 44.0, this.Red(), true);

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
        let cls = Reflection.GetClass(n"MenuScenario_PauseMenu");
        while IsDefined(cls) {
            for func in cls.GetFunctions() {
                WheelDriveTest_Event(s"menu-function: \(NameToString(cls.GetName())).\(NameToString(func.GetName()))");
            }
            cls = cls.GetParent();
        }
    }

    protected cb func OnBoostDown(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            WheelDriveTest_BoostAdjust(-1);
        }
    }

    protected cb func OnBoostUp(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            WheelDriveTest_BoostAdjust(1);
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
            this.m_boostValue = null;
            this.m_boostFill = null;
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
            if WheelDriveTest_ScreenRowLit(row) {
                this.m_rowMarks[row].SetOpacity(0.7);
            } else {
                this.m_rowMarks[row].SetOpacity(row == selected ? 0.22 : 0.03);
            }
            row += 1;
        }
        let percent = WheelDriveTest_BoostPercent();
        this.m_boostValue.SetText(s"\(percent)%");
        this.m_boostFill.SetSize(new Vector2(460.0 * Cast<Float>(percent) / 200.0, 56.0));
        this.m_status.SetText(WheelDriveTest_ScreenStatus());
        this.m_hints.SetText(WheelDriveTest_ScreenHints());
    }
}
