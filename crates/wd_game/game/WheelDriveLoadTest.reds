// First game load test. Reports events to the plugin, which logs them. Changes nothing in the game.

native func WheelDriveTest_Ping(value: Int32) -> Int32;
native func WheelDriveTest_Event(name: String);

public class WheelDriveTestService extends ScriptableService {
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
    }

    private cb func OnSessionPause(event: ref<GameSessionEvent>) {
        WheelDriveTest_Event("session-pause");
    }

    private cb func OnSessionResume(event: ref<GameSessionEvent>) {
        WheelDriveTest_Event("session-resume");
    }

    private cb func OnSessionEnd(event: ref<GameSessionEvent>) {
        WheelDriveTest_Event("session-end");
    }

    // Records that a press arrived, never which key.
    private cb func OnKey(event: ref<KeyInputEvent>) {
        switch event.GetAction() {
            case EInputAction.IACT_Press:
                WheelDriveTest_Event("key-press");
                break;
            default:
                break;
        }
    }
}
