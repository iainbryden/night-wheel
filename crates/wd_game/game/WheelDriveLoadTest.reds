// Game test. Tells the plugin each frame whether the player is driving, because the wheel must
// only apply then. During pause it shows the live wheel value and a value the comma and period
// keys change. On first driving it reports the vehicle's steering settings, using read-only
// getters and property reads, and it passes the game's steering action to the plugin.

native func WheelDriveTest_Ping(value: Int32) -> Int32;
native func WheelDriveTest_Event(name: String);
native func WheelDriveTest_Read() -> Int32;
native func WheelDriveTest_Key() -> Int32;
native func WheelDriveTest_SetDriving(driving: Bool);
native func WheelDriveTest_SteeringAction(value: Float);

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
    private let m_reported: Bool;
    private let m_listening: wref<PlayerPuppet>;
    private let m_paused: Bool;
    private let m_steering: Int32;
    private let m_keys: Int32;
    private let m_testValue: Int32;
    private let m_text: ref<inkText>;
    private let m_refresh: ref<inkAnimProxy>;

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
        this.ShowText();
    }

    private cb func OnSessionResume(event: ref<GameSessionEvent>) {
        this.m_paused = false;
        WheelDriveTest_Event("session-resume");
        this.HideText();
    }

    private cb func OnSessionEnd(event: ref<GameSessionEvent>) {
        this.m_active = false;
        this.m_paused = false;
        this.m_driving = false;
        this.m_listening = null;
        WheelDriveTest_SetDriving(false);
        WheelDriveTest_Event("session-end");
        this.HideText();
    }

    // Records that a press arrived, and never which key outside the two test keys.
    private cb func OnKey(event: ref<KeyInputEvent>) {
        if NotEquals(event.GetAction(), EInputAction.IACT_Press) {
            return;
        }
        this.m_keys = WheelDriveTest_Key();
        if this.m_paused {
            switch event.GetKey() {
                case EInputKey.IK_Comma:
                    this.m_testValue -= 1;
                    break;
                case EInputKey.IK_Period:
                    this.m_testValue += 1;
                    break;
                default:
                    break;
            }
            this.UpdateText();
        }
    }

    public func OnTick() {
        if !this.m_active {
            return;
        }
        this.m_steering = WheelDriveTest_Read();
        this.UpdateDriving();
        this.UpdateText();
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
                this.ReportVehicle(player);
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

    private func ReportVehicle(player: ref<PlayerPuppet>) {
        if this.m_reported {
            return;
        }
        this.m_reported = true;
        this.ReportDeclared(n"gamedataVehicleSteeringSettings_Record");
        this.ReportDeclared(n"gamedataVehicleDriveModelData_Record");
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) {
            WheelDriveTest_Event("report: no vehicle found");
            return;
        }
        this.ReportProperties(vehicle);
        this.ReportRecord("vehicle", vehicle.GetRecord(), 0);
    }

    // Lists what a record type offers, whether or not it can be reached from this vehicle.
    private func ReportDeclared(name: CName) {
        let cls = Reflection.GetClass(name);
        if !IsDefined(cls) {
            WheelDriveTest_Event(s"report: no class \(NameToString(name))");
            return;
        }
        for func in cls.GetFunctions() {
            let type = func.GetReturnType();
            let returns = IsDefined(type) ? NameToString(type.GetName()) : "nothing";
            WheelDriveTest_Event(s"report: \(NameToString(name)).\(NameToString(func.GetName())) -> \(returns)");
        }
    }

    private func ReportProperties(vehicle: ref<VehicleObject>) {
        let owner = ToVariant(vehicle);
        let cls = Reflection.GetClassOf(owner, true);
        while IsDefined(cls) {
            for prop in cls.GetProperties() {
                let typeName = prop.GetType().GetName();
                if this.IsPlainValue(typeName) {
                    WheelDriveTest_Event(s"report: \(NameToString(cls.GetName())).\(NameToString(prop.GetName())) = \(this.ValueText(typeName, prop.GetValue(owner)))");
                }
            }
            cls = cls.GetParent();
        }
    }

    // Calls only no-argument getters on game data records, which only return stored values.
    private func ReportRecord(label: String, record: ref<IScriptable>, depth: Int32) {
        if !IsDefined(record) || depth > 2 {
            return;
        }
        let cls = Reflection.GetClassOf(ToVariant(record), true);
        for func in cls.GetFunctions() {
            let type = func.GetReturnType();
            let parameters = func.GetParameters();
            if IsDefined(type) && ArraySize(parameters) == 0 {
                let typeName = type.GetName();
                let returns = NameToString(typeName);
                let name = s"\(label).\(NameToString(func.GetName()))";
                if StrContains(returns, "VehicleDriveModelData_Record") || StrContains(returns, "VehicleSteeringSettings_Record") {
                    this.ReportRecord(name, FromVariant<ref<IScriptable>>(func.Call(record)), depth + 1);
                } else {
                    if depth > 0 && this.IsPlainValue(typeName) {
                        WheelDriveTest_Event(s"report: \(name) = \(this.ValueText(typeName, func.Call(record)))");
                    }
                }
            }
        }
    }

    private func IsPlainValue(type: CName) -> Bool {
        return Equals(type, n"Float") || Equals(type, n"Int32") || Equals(type, n"Bool");
    }

    private func ValueText(type: CName, value: Variant) -> String {
        switch type {
            case n"Float":
                return FloatToString(FromVariant<Float>(value));
            case n"Int32":
                return IntToString(FromVariant<Int32>(value));
            case n"Bool":
                return BoolToString(FromVariant<Bool>(value));
            default:
                return "";
        }
    }

    private func ScheduleTick() {
        let tick = new WheelDriveTestTick();
        tick.service = this;
        GameInstance.GetDelaySystem(GetGameInstance()).DelayCallback(tick, 0.0, false);
    }

    private func ShowText() {
        if IsDefined(this.m_text) {
            return;
        }
        let layer = GameInstance.GetInkSystem().GetLayer(n"inkMenuLayer");
        if !IsDefined(layer) {
            WheelDriveTest_Event("menu-layer-missing");
            return;
        }
        let text = new inkText();
        text.SetFontFamily("base\\gameplay\\gui\\fonts\\orbitron\\orbitron.inkfontfamily");
        text.SetFontStyle(n"Bold");
        text.SetFontSize(48);
        text.SetTintColor(new HDRColor(1.1761, 0.3809, 0.3476, 1.0));
        text.SetAnchor(inkEAnchor.TopCenter);
        text.SetAnchorPoint(0.5, 0.0);
        text.SetMargin(new inkMargin(0.0, 40.0, 0.0, 0.0));
        text.Reparent(layer.GetVirtualWindow());
        this.m_text = text;
        this.UpdateText();
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

        this.m_refresh = this.m_text.PlayAnimationWithOptions(definition, options);
        this.m_refresh.RegisterToCallback(inkanimEventType.OnEndLoop, this, n"OnRefresh");
    }

    private cb func OnRefresh(proxy: ref<inkAnimProxy>) -> Bool {
        this.m_steering = WheelDriveTest_Read();
        this.UpdateText();
    }

    private func HideText() {
        if IsDefined(this.m_refresh) {
            this.m_refresh.Stop();
            this.m_refresh = null;
        }
        if IsDefined(this.m_text) {
            this.m_text.SetVisible(false);
            this.m_text = null;
        }
    }

    private func UpdateText() {
        if IsDefined(this.m_text) {
            this.m_text.SetText(s"WHEEL TEST  X \(this.m_steering)  KEYS \(this.m_keys)  VALUE \(this.m_testValue)");
        }
    }
}
