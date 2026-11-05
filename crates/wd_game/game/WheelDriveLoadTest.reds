// Game test. Tells the plugin each frame whether the player is driving, because the wheel must
// only apply then. During pause it shows the live wheel value and a value the comma and period
// keys change.

native func WheelDriveTest_Ping(value: Int32) -> Int32;
native func WheelDriveTest_Event(name: String);
native func WheelDriveTest_Read() -> Int32;
native func WheelDriveTest_Key() -> Int32;
native func WheelDriveTest_SetDriving(driving: Bool);

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
