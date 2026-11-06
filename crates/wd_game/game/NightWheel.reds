// The game side of the mod. Each frame it tells the plugin whether the player is driving, because
// the wheel only applies then, and it carries out the functions that have no gamepad button: radio
// controls, boost and flight. Four steering speed values are multiplied in memory when the game
// starts, for every vehicle drive model, by an amount the plugin supplies.
// The WHEEL SETTINGS pause menu entry opens a full page, laid out like the game's Key Bindings page.
// The plugin runs that page, and this script passes it input and draws it. While it is open the
// pause menu is hidden and its entries do nothing. Nothing here changes a save.

native func NightWheel_Event(name: String);
native func NightWheel_SetDriving(driving: Bool);
native func NightWheel_SteeringAction(value: Float);
native func NightWheel_SteeringSpeed() -> Float;
native func NightWheel_ScreenOpen();
native func NightWheel_ScreenClose();
native func NightWheel_ScreenCommand(code: Int32);
native func NightWheel_ScreenTick() -> Bool;
native func NightWheel_ScreenRowCount() -> Int32;
native func NightWheel_ScreenRowLabel(row: Int32) -> String;
native func NightWheel_ScreenRowValue(row: Int32) -> String;
native func NightWheel_ScreenSelected() -> Int32;
native func NightWheel_ScreenSelect(row: Int32);
native func NightWheel_ScreenRowLit(row: Int32) -> Bool;
native func NightWheel_ScreenSection(row: Int32) -> String;
native func NightWheel_TakeScriptActions() -> Int32;
native func NightWheel_ScreenColumn(row: Int32) -> Int32;
native func NightWheel_ScreenBoostRow() -> Int32;
native func NightWheel_BoostPercent() -> Int32;
native func NightWheel_BoostAdjust(steps: Int32);
native func NightWheel_BoostWanted() -> Bool;
native func NightWheel_BoostHeld() -> Bool;
native func NightWheel_ScriptHeld() -> Int32;
native func NightWheel_Throttle() -> Float;
native func NightWheel_Brake() -> Float;
native func NightWheel_SetAutopilot(on: Bool);
native func NightWheel_ScreenStatus() -> String;
native func NightWheel_SliderCount() -> Int32;
native func NightWheel_SliderLabel(index: Int32) -> String;
native func NightWheel_SliderValue(index: Int32) -> String;
native func NightWheel_SliderFraction(index: Int32) -> Float;
native func NightWheel_SliderAdjust(index: Int32, steps: Int32);
native func NightWheel_ScreenHints() -> String;

public class NightWheelSteeringRates extends ScriptableTweak {
    protected cb func OnApply() -> Void {
        let multiplier = NightWheel_SteeringSpeed();
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
        NightWheel_Event(s"startup rates: \(ArraySize(records)) drive models, \(changed) values multiplied by \(FloatToString(multiplier))");
    }
}

// Adds the WHEEL SETTINGS entry after the game's own entries, and leaves those entries to the game.
@wrapMethod(PauseMenuGameController)
private func PopulateMenuItemList() -> Void {
    wrappedMethod();
    this.AddMenuItem(NightWheelService.EntryLabel(), n"OnNightWheelOpen");
    this.m_menuListController.Refresh();
    NightWheel_Event("wheel-entry-added");
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"NightWheelService") as NightWheelService;
    if IsDefined(service) {
        service.SetMenuRoot(this.GetRootWidget());
        service.SetMenuController(this);
    }
}

@wrapMethod(PauseMenuGameController)
protected cb func OnMenuItemActivated(index: Int32, target: ref<ListItemController>) -> Bool {
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"NightWheelService") as NightWheelService;
    if IsDefined(service) && service.IsScreenOpen() {
        return true;
    }
    let data = target.GetData() as PauseMenuListItemData;
    if IsDefined(data) && Equals(data.label, NightWheelService.EntryLabel()) {
        NightWheel_Event("wheel-entry-activated");
        if IsDefined(service) {
            service.OpenScreen();
        } else {
            NightWheel_Event("wheel-service-missing");
        }
        return true;
    }
    return wrappedMethod(index, target);
}

// Going back cancels a bind or leaves the page, in place of closing the pause menu.
@wrapMethod(PauseMenuGameController)
protected cb func OnGlobalRelease(e: ref<inkPointerEvent>) -> Bool {
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"NightWheelService") as NightWheelService;
    if IsDefined(service) && service.IsScreenOpen() && e.IsAction(n"back") {
        NightWheel_Event("wheel-back");
        NightWheel_ScreenCommand(5);
        return true;
    }
    return wrappedMethod(e);
}

// The same for the pause menu scenario's own back handler.
@wrapMethod(MenuScenario_PauseMenu)
protected cb func OnBack() -> Bool {
    let service = GameInstance.GetScriptableServiceContainer().GetService(n"NightWheelService") as NightWheelService;
    if IsDefined(service) && service.IsScreenOpen() {
        NightWheel_Event("wheel-back-scenario");
        NightWheel_ScreenCommand(5);
        return true;
    }
    return wrappedMethod();
}

public class NightWheelTick extends DelayCallback {
    public let service: wref<NightWheelService>;

    public func Call() {
        if IsDefined(this.service) {
            this.service.OnTick();
        }
    }
}

public class NightWheelService extends ScriptableService {
    private let m_active: Bool;
    private let m_driving: Bool;
    private let m_listening: wref<PlayerPuppet>;
    private let m_paused: Bool;
    private let m_refreshHost: ref<inkText>;
    private let m_refresh: ref<inkAnimProxy>;
    private let m_screen: ref<inkCanvas>;
    private let m_menuRoot: wref<inkWidget>;
    private let m_rowValues: array<ref<inkText>>;
    private let m_rowMarks: array<ref<inkRectangle>>;
    private let m_status: ref<inkText>;
    private let m_hints: ref<inkText>;
    private let m_inMenu: Bool;
    private let m_turn: Float;
    private let m_flying: Bool;
    private let m_flightVehicle: EntityID;
    private let m_flightStart: Float;
    private let m_flightTarget: Float;
    private let m_flightLastTime: Float;
    private let m_flightLastYaw: Float;
    private let m_flightYawRate: Float;
    private let m_flightX: Float;
    private let m_flightY: Float;
    private let m_flightZ: Float;
    private let m_flightVX: Float;
    private let m_flightVY: Float;
    private let m_flightVZ: Float;
    private let m_carrying: Bool;
    private let m_carryVehicle: EntityID;
    private let m_carrySpeed: Float;
    private let m_carryLast: Float;
    private let m_carryEnd: Float;
    private let m_carryLogged: Float;
    private let m_flightClimbHeld: Bool;
    private let m_flightGroundHeld: Float;
    private let m_lastBoostEnd: Float;
    private let m_autopilot: Bool;
    private let m_boosting: Bool;
    private let m_boostButtonHeld: Bool;
    private let m_boostBurstStart: Float;
    private let m_boostCooldownEnd: Float;
    private let m_boostLimitSet: Bool;
    private let m_boostPeak: Float;
    private let m_boostLimit: Float;
    private let m_topSeen: Float;
    private let m_boostVehicle: EntityID;
    private let m_lastBoostTime: Float;
    private let m_boostValue: ref<inkText>;
    private let m_boostFill: ref<inkRectangle>;
    private let m_sliderValues: array<ref<inkText>>;
    private let m_sliderFills: array<ref<inkRectangle>>;
    private let m_sliderControls: array<ref<inkSliderController>>;
    private let m_sliderShown: array<Float>;
    private let m_menuController: wref<inkGameController>;

    public static func EntryLabel() -> String {
        return "WHEEL SETTINGS";
    }

    private cb func OnLoad() {
        NightWheel_Event("script-loaded");

        let callbacks = GameInstance.GetCallbackSystem();
        callbacks.RegisterCallback(n"Session/Ready", this, n"OnSessionReady");
        callbacks.RegisterCallback(n"Session/Pause", this, n"OnSessionPause");
        callbacks.RegisterCallback(n"Session/Resume", this, n"OnSessionResume");
        callbacks.RegisterCallback(n"Session/End", this, n"OnSessionEnd");
        callbacks.RegisterCallback(n"Input/Key", this, n"OnKey");
    }

    private cb func OnSessionReady(event: ref<GameSessionEvent>) {
        NightWheel_Event("session-ready");
        this.ResetVehicleState("session start");
        if !this.m_active {
            this.m_active = true;
            this.ScheduleTick();
        }
    }

    private cb func OnSessionPause(event: ref<GameSessionEvent>) {
        this.m_paused = true;
        NightWheel_Event("session-pause");
        this.StartPauseUpdates();
    }

    private cb func OnSessionResume(event: ref<GameSessionEvent>) {
        this.m_paused = false;
        NightWheel_Event("session-resume");
        this.HideAll();
    }

    private cb func OnSessionEnd(event: ref<GameSessionEvent>) {
        this.m_active = false;
        this.m_paused = false;
        this.m_driving = false;
        this.m_listening = null;
        this.ResetVehicleState("session end");
        NightWheel_SetDriving(false);
        NightWheel_Event("session-end");
        this.HideAll();
    }

    // Boost, flight and Autodrive belong to one session. Flight's saved position must never move a
    // car in the next one.
    private func ResetVehicleState(reason: String) {
        this.EndBoost(reason);
        this.EndFlight(reason);
        this.m_flightClimbHeld = false;
        this.m_carrying = false;
        this.m_boostButtonHeld = false;
        // Game time may restart with a session, and an old end time could block boost for too long.
        this.m_boostCooldownEnd = 0.0;
        if this.m_autopilot {
            this.m_autopilot = false;
            NightWheel_SetAutopilot(false);
        }
    }

    // Keys only matter while the settings page is open.
    private cb func OnKey(event: ref<KeyInputEvent>) {
        if NotEquals(event.GetAction(), EInputAction.IACT_Press) || !this.m_paused || !this.IsScreenOpen() {
            return;
        }
        switch event.GetKey() {
            case EInputKey.IK_Up:
                NightWheel_ScreenCommand(1);
                break;
            case EInputKey.IK_Down:
                NightWheel_ScreenCommand(2);
                break;
            case EInputKey.IK_Enter:
                NightWheel_ScreenCommand(3);
                break;
            case EInputKey.IK_Delete:
                NightWheel_ScreenCommand(4);
                break;
            case EInputKey.IK_Period:
                NightWheel_ScreenCommand(5);
                break;
            case EInputKey.IK_Left:
                NightWheel_BoostAdjust(-1);
                break;
            case EInputKey.IK_Right:
                NightWheel_BoostAdjust(1);
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
            NightWheel_Event(s"in-menu: \(inMenu)");
        }
        let driving = !inMenu && IsDefined(player) && VehicleComponent.IsDriver(game, player);
        NightWheel_SetDriving(driving);
        if driving {
            let actions = NightWheel_TakeScriptActions();
            if actions != 0 {
                this.RunRadioActions(player, actions);
            }
            this.UpdateAutopilot(player);
            this.UpdateBoostBurst();
            this.UpdateBoost(player);
            this.UpdateFlight(player);
            this.UpdateLandingCarry(player);
        } else {
            this.EndBoost("not driving");
            this.m_carrying = false;
            if !inMenu {
                this.EndFlight("not driving");
            }
            if this.m_autopilot {
                this.m_autopilot = false;
                NightWheel_SetAutopilot(false);
            }
        }
        if NotEquals(driving, this.m_driving) {
            this.m_driving = driving;
            NightWheel_Event(driving ? "driving-on" : "driving-off");
            if driving {
                this.StartListening(player);
            }
        }
    }

    // The game's own driving must not have the wheel steering against it. The flag's name comes
    // from the game's vehicle blackboard. Whether it follows Autodrive isn't confirmed in play.
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
            NightWheel_SetAutopilot(on);
            NightWheel_Event(s"autopilot: \(on)");
        }
    }

    // A boost burst lasts up to 5 seconds, and any end to it starts a 3 second cooldown. A new burst
    // takes a fresh press of the button once the cooldown is over, so tapping can't get round the
    // limit and holding can't restart it. Driving and flight share this one timer, so switching
    // between them can't reset it. Both times are provisional.
    private func UpdateBoostBurst() {
        let now = this.SimNow();
        let held = NightWheel_BoostHeld();
        let pressed = held && !this.m_boostButtonHeld;
        this.m_boostButtonHeld = held;
        let wanted = NightWheel_BoostWanted() && !this.m_autopilot;
        if this.m_boosting {
            if !wanted {
                this.EndBoost("released");
            } else {
                if now - this.m_boostBurstStart >= 5.0 {
                    this.EndBoost("time limit");
                }
            }
        } else {
            if pressed && wanted && now >= this.m_boostCooldownEnd {
                this.m_boosting = true;
                this.m_boostLimitSet = false;
                this.m_boostBurstStart = now;
                NightWheel_Event("boost: burst on");
            }
        }
    }

    private func SimNow() -> Float {
        return EngineTime.ToFloat(GameInstance.GetSimTime(GetGameInstance()));
    }

    // During a burst, boost pushes the car forward through the physics system, up to a limit above
    // the fastest speed seen without boost. In flight, thrust is added in UpdateFlight instead. It
    // changes no vehicle data and saves nothing. The push strength is a provisional value.
    private func UpdateBoost(player: ref<PlayerPuppet>) {
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) || this.m_flying {
            return;
        }
        let id = vehicle.GetEntityID();
        if NotEquals(id, this.m_boostVehicle) {
            this.m_boostVehicle = id;
            this.m_topSeen = 0.0;
            this.EndBoost("new vehicle");
        }
        let speed = vehicle.GetCurrentSpeed();
        let now = this.SimNow();
        let elapsed = now - this.m_lastBoostTime;
        this.m_lastBoostTime = now;
        if !this.m_boosting {
            if speed > this.m_topSeen && now - this.m_lastBoostEnd > 15.0 {
                this.m_topSeen = speed;
            }
            return;
        }
        let percent = Cast<Float>(NightWheel_BoostPercent());
        if !this.m_boostLimitSet {
            this.m_boostLimitSet = true;
            this.m_boostPeak = speed;
            // With no fast speed seen yet, a press at a crawl still needs room to accelerate.
            let reference = MaxF(MaxF(this.m_topSeen, speed), 20.0);
            this.m_boostLimit = reference * (1.0 + percent / 100.0);
            NightWheel_Event(s"boost: pushing from \(FloatToString(speed)), limit \(FloatToString(this.m_boostLimit)), fastest seen \(FloatToString(this.m_topSeen)), strength \(FloatToString(percent))");
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

    // Flight mode sets the car's position and heading each frame through the game's teleport
    // function, so gravity and tire physics no longer decide where it goes. The mod simulates
    // motion: the wheel sets a turn rate, the pedals give thrust and braking, and momentum carries.
    // Climb starts flight, and holding Descend at the ground ends it. A ray down to the ground stops
    // it going below ground. Nothing here checks for buildings ahead, so it can pass through them.
    // Every strength is a provisional value. It changes no vehicle data and saves nothing.
    private func UpdateFlight(player: ref<PlayerPuppet>) {
        let vehicle = GetMountedVehicle(player);
        if !IsDefined(vehicle) {
            return;
        }
        // Flight's position belongs to one car.
        if this.m_flying && NotEquals(vehicle.GetEntityID(), this.m_flightVehicle) {
            this.EndFlight("vehicle changed");
        }
        let game = GetGameInstance();
        // A fresh press of Climb starts flight when it is off.
        let held = NightWheel_ScriptHeld();
        let climbHeld = (held & 128) != 0;
        let descendHeld = (held & 256) != 0;
        let climbPressed = climbHeld && !this.m_flightClimbHeld;
        this.m_flightClimbHeld = climbHeld;
        if climbPressed && !this.m_autopilot && !this.m_flying {
            this.StartFlight(vehicle);
        }
        let now = EngineTime.ToFloat(GameInstance.GetSimTime(game));
        let elapsed = MinF(now - this.m_flightLastTime, 0.1);
        this.m_flightLastTime = now;
        if !this.m_flying || elapsed <= 0.0 {
            return;
        }
        if this.m_autopilot {
            this.EndFlight("autodrive");
            return;
        }

        // Turning.
        this.m_flightYawRate += (-this.m_turn * 60.0 - this.m_flightYawRate) * MinF(elapsed * 6.0, 1.0);
        let yaw = this.m_flightLastYaw + this.m_flightYawRate * elapsed;
        this.m_flightLastYaw = yaw;
        let forward = vehicle.GetWorldForward();
        let right = vehicle.GetWorldRight();
        let flat = SqrtF(forward.X * forward.X + forward.Y * forward.Y);
        let aheadX = flat > 0.1 ? forward.X / flat : 1.0;
        let aheadY = flat > 0.1 ? forward.Y / flat : 0.0;
        let sideX = aheadY;
        let sideY = -aheadX;
        if right.X * sideX + right.Y * sideY < 0.0 {
            sideX = -sideX;
            sideY = -sideY;
        }

        // Motion along the heading and across it. Sideways motion fades, so the car goes where it
        // points, and a little drag slows it when coasting.
        let along = this.m_flightVX * aheadX + this.m_flightVY * aheadY;
        let across = this.m_flightVX * sideX + this.m_flightVY * sideY;
        let boostPercent = Cast<Float>(NightWheel_BoostPercent());
        let boosting = this.m_boosting;
        let thrust = NightWheel_Throttle() * 15.0;
        if boosting {
            thrust += MinF(6.0 + boostPercent * 0.12, 30.0);
        }
        along += (thrust - NightWheel_Brake() * 25.0) * elapsed;
        along -= along * 0.1 * elapsed;
        let limit = boosting ? 70.0 * (1.0 + boostPercent / 100.0) : 70.0;
        along = ClampF(along, 0.0, MaxF(limit, along - 30.0 * elapsed));
        across -= across * MinF(elapsed * 2.0, 1.0);
        this.m_flightVX = aheadX * along + sideX * across;
        this.m_flightVY = aheadY * along + sideY * across;

        // Height.
        if climbHeld {
            this.m_flightTarget += 15.0 * elapsed;
        }
        if descendHeld {
            this.m_flightTarget -= 15.0 * elapsed;
        }
        this.m_flightTarget = ClampF(this.m_flightTarget, this.m_flightStart - 400.0, this.m_flightStart + 400.0);
        this.m_flightTarget = ClampF(this.m_flightTarget, this.m_flightZ - 15.0, this.m_flightZ + 15.0);
        let wantedRise = ClampF((this.m_flightTarget - this.m_flightZ) * 2.0, -15.0, 15.0);
        this.m_flightVZ += (wantedRise - this.m_flightVZ) * MinF(elapsed * 4.0, 1.0);

        // Move.
        let x = this.m_flightX + this.m_flightVX * elapsed;
        let y = this.m_flightY + this.m_flightVY * elapsed;
        let z = this.m_flightZ + this.m_flightVZ * elapsed;

        // The ground below.
        let from = new Vector4(x, y, z + 1.0, 1.0);
        let to = new Vector4(x, y, z - 200.0, 1.0);
        let groundZ = MaxF(this.GroundHeight(from, to, n"Static"), this.GroundHeight(from, to, n"Terrain"));
        let groundKnown = groundZ > -99999.0;
        let clearance = 999.0;
        if groundKnown {
            clearance = z - groundZ;
            if clearance < 0.8 {
                z = groundZ + 0.8;
                this.m_flightTarget = MaxF(this.m_flightTarget, z);
                if this.m_flightVZ < 0.0 {
                    this.m_flightVZ = 0.0;
                }
            }
        }
        // Holding Descend at the ground for half a second ends flight. A ray has to see the
        // ground, so this never drops the car from the air.
        if descendHeld && clearance < 1.2 {
            this.m_flightGroundHeld += elapsed;
            if this.m_flightGroundHeld >= 0.5 {
                this.EndFlight("descend held at ground");
                this.StartLandingCarry(vehicle, along);
                return;
            }
        } else {
            this.m_flightGroundHeld = 0.0;
        }
        this.m_flightX = x;
        this.m_flightY = y;
        this.m_flightZ = z;

        let facing: EulerAngles;
        facing.Pitch = 0.0;
        facing.Roll = 0.0;
        facing.Yaw = yaw;
        GameInstance.GetTeleportationFacility(game).Teleport(vehicle, new Vector4(x, y, z, 1.0), facing);

        // Cancels the gravity that would otherwise build up between frames.
        this.Push(vehicle, vehicle.GetWorldPosition(), 0.0, 0.0, vehicle.GetTotalMass() * 9.81 * elapsed);
    }

    // The physics barely see the car move during flight, so on landing it would stop dead. For one
    // second this wakes the physics and pushes up to the speed it flew at, measuring the real speed
    // each frame so a push that falls short is made up. The speed is capped at what the car has
    // reached on the road, or 40, so a fast landing can't launch it. Braking ends it early. The cap,
    // time and strength are provisional, and each step is logged for tuning.
    private func StartLandingCarry(vehicle: ref<VehicleObject>, along: Float) {
        this.m_carrying = true;
        this.m_carryVehicle = vehicle.GetEntityID();
        this.m_carrySpeed = MinF(along, MaxF(this.m_topSeen, 40.0));
        this.m_carryLast = this.SimNow();
        this.m_carryEnd = this.m_carryLast + 1.0;
        this.m_carryLogged = 0.0;
        NightWheel_Event(s"landing: flew at \(FloatToString(along)), holding \(FloatToString(this.m_carrySpeed))");
    }

    private func UpdateLandingCarry(player: ref<PlayerPuppet>) {
        if !this.m_carrying {
            return;
        }
        let vehicle = GetMountedVehicle(player);
        let now = this.SimNow();
        let elapsed = MinF(now - this.m_carryLast, 0.1);
        if !IsDefined(vehicle) || this.m_flying || now >= this.m_carryEnd || NightWheel_Brake() > 0.1 {
            this.m_carrying = false;
            NightWheel_Event("landing: done");
            return;
        }
        if NotEquals(vehicle.GetEntityID(), this.m_carryVehicle) {
            this.m_carrying = false;
            return;
        }
        if elapsed <= 0.0 {
            return;
        }
        this.m_carryLast = now;
        vehicle.PhysicsWakeUp();
        let forward = vehicle.GetWorldForward();
        let flat = SqrtF(forward.X * forward.X + forward.Y * forward.Y);
        if flat < 0.1 {
            return;
        }
        let aheadX = forward.X / flat;
        let aheadY = forward.Y / flat;
        let moving = vehicle.GetLinearVelocity();
        let speed = moving.X * aheadX + moving.Y * aheadY;
        let change = ClampF(this.m_carrySpeed - speed, 0.0, 240.0 * elapsed) * vehicle.GetTotalMass();
        if change > 0.0 {
            this.Push(vehicle, vehicle.GetWorldPosition(), aheadX * change, aheadY * change, 0.0);
        }
        if now - this.m_carryLogged >= 0.05 {
            this.m_carryLogged = now;
            NightWheel_Event(s"landing: speed \(FloatToString(speed)), pushing \(FloatToString(change))");
        }
    }

    // The height a ray hits in one collision group, or a very low value if it hits nothing.
    private func GroundHeight(from: Vector4, to: Vector4, group: CName) -> Float {
        let trace: TraceResult;
        let hit = GameInstance.GetSpatialQueriesSystem(GetGameInstance()).SyncRaycastByCollisionGroup(from, to, group, trace, true, false);
        if hit && TraceResult.IsValid(trace) {
            let point = trace.position;
            return point.Z;
        }
        return -100000.0;
    }

    private func StartFlight(vehicle: ref<VehicleObject>) {
        let begin = vehicle.GetWorldPosition();
        let moving = vehicle.GetLinearVelocity();
        this.m_flying = true;
        this.m_flightVehicle = vehicle.GetEntityID();
        this.m_flightStart = begin.Z;
        this.m_flightTarget = begin.Z + 3.0;
        this.m_flightX = begin.X;
        this.m_flightY = begin.Y;
        this.m_flightZ = begin.Z;
        this.m_flightVX = moving.X;
        this.m_flightVY = moving.Y;
        this.m_flightVZ = 0.0;
        this.m_flightLastYaw = vehicle.GetWorldYaw();
        this.m_flightYawRate = 0.0;
        this.m_flightGroundHeld = 0.0;
        NightWheel_Event(s"flight: on at height \(FloatToString(begin.Z)), speed \(FloatToString(vehicle.GetCurrentSpeed()))");
    }

    private func Push(vehicle: ref<VehicleObject>, at: Vector4, x: Float, y: Float, z: Float) {
        let event = new PhysicalImpulseEvent();
        event.radius = 1.0;
        event.worldPosition = Vector4.Vector4To3(at);
        event.worldImpulse = new Vector3(x, y, z);
        vehicle.QueueEvent(event);
    }

    private func EndFlight(reason: String) {
        if this.m_flying {
            this.m_flying = false;
            NightWheel_Event(s"flight: off, \(reason)");
        }
    }

    private func EndBoost(reason: String) {
        if this.m_boosting {
            this.m_boosting = false;
            this.m_boostCooldownEnd = this.SimNow() + 3.0;
            NightWheel_Event(s"boost: off, \(reason), peak \(FloatToString(this.m_boostPeak))");
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
            NightWheel_Event(s"radio: was on=\(on), toggled");
        }
        if (actions & 2) != 0 {
            vehicle.NextRadioReceiverStation();
            NightWheel_Event(s"radio: next, index \(vehicle.GetCurrentRadioIndex())");
        }
        if (actions & 4) != 0 {
            let index = Cast<Int32>(vehicle.GetCurrentRadioIndex());
            let position = RadioStationDataProvider.GetRadioStationUIIndex(index);
            if position > 0 {
                let previous = EnumInt(RadioStationDataProvider.GetRadioStationByUIIndex(position - 1));
                vehicle.SetRadioReceiverStation(Cast<Uint32>(previous));
            }
            NightWheel_Event(s"radio: previous from position \(position), index \(vehicle.GetCurrentRadioIndex())");
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
            NightWheel_Event("radio: volume setting not found");
            return;
        }
        let value = volume.GetValue() + direction * volume.GetStepValue();
        value = Max(volume.GetMinValue(), Min(volume.GetMaxValue(), value));
        volume.SetValue(value);
        if settings.NeedsConfirmation() {
            settings.ConfirmChanges();
        }
        NightWheel_Event(s"radio: volume \(value)");
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
            NightWheel_Event("listening-for-steering-action");
        }
    }

    protected cb func OnAction(action: ListenerAction, consumer: ListenerActionConsumer) -> Bool {
        if Equals(ListenerAction.GetName(action), n"TurnX") {
            this.m_turn = ListenerAction.GetValue(action);
            NightWheel_SteeringAction(this.m_turn);
        }
    }

    private func ScheduleTick() {
        let tick = new NightWheelTick();
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

    private func StartPauseUpdates() {
        if IsDefined(this.m_refreshHost) {
            return;
        }
        let layer = this.MenuLayer();
        if !IsDefined(layer) {
            NightWheel_Event("menu-layer-missing");
            return;
        }
        let host = this.NewText(32);
        host.SetAnchor(inkEAnchor.TopCenter);
        host.SetAnchorPoint(0.5, 0.0);
        host.SetMargin(new inkMargin(0.0, 40.0, 0.0, 0.0));
        host.SetText("");
        host.Reparent(layer.GetVirtualWindow());
        this.m_refreshHost = host;
        this.StartRefresh();
    }

    // The delay system mostly stops during pause, so a looping UI animation drives the display.
    // It plays on an empty text that shows nothing, and only supplies the update.
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

        this.m_refresh = this.m_refreshHost.PlayAnimationWithOptions(definition, options);
        this.m_refresh.RegisterToCallback(inkanimEventType.OnEndLoop, this, n"OnRefresh");
    }

    private cb func OnRefresh(proxy: ref<inkAnimProxy>) -> Bool {
        this.UpdateScreen();
    }

    public func IsScreenOpen() -> Bool {
        return IsDefined(this.m_screen);
    }

    public func SetMenuController(controller: ref<inkGameController>) {
        this.m_menuController = controller;
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
            NightWheel_Event("menu-layer-missing");
            return;
        }
        NightWheel_ScreenOpen();

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
        let count = NightWheel_ScreenRowCount();
        let boostRow = NightWheel_ScreenBoostRow();
        let columnY: array<Float>;
        ArrayPush(columnY, 100.0);
        ArrayPush(columnY, 100.0);
        let columnW = 1280.0;
        let row = 0;
        while row < count {
            let column = NightWheel_ScreenColumn(row);
            let x = Cast<Float>(column) * (columnW + 80.0);
            let y = columnY[column];
            let section = NightWheel_ScreenSection(row);
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

            this.AddText(content, body, 36, x + 20.0, y, 620.0, 68.0, this.Red(), false).SetText(NightWheel_ScreenRowLabel(row));

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

        // The steering section, below both columns and as wide as the game's own settings rows.
        this.ClearSliders();
        let sliderW = 1800.0;
        let sliderX = (2640.0 - sliderW) / 2.0;
        let sliderY = MaxF(columnY[0], columnY[1]) + 16.0;
        let sliderBand = this.AddRect(content, sliderX, sliderY, sliderW, 62.0, new HDRColor(0.05, 0.08, 0.11, 1.0));
        sliderBand.SetOpacity(0.9);
        this.AddRect(content, sliderX, sliderY, sliderW, 3.0, this.Teal());
        this.AddText(content, body, 38, sliderX + 20.0, sliderY, sliderW - 40.0, 62.0, new HDRColor(0.9, 0.9, 0.9, 1.0), false).SetText("Steering");
        sliderY += 70.0;
        let slider = 0;
        while slider < NightWheel_SliderCount() {
            if !this.AddGameSlider(content, slider, sliderX, sliderY) {
                this.AddSlider(content, body, slider, sliderX, sliderY);
            }
            sliderY += 88.0;
            slider += 1;
        }
        columnY[1] = sliderY;

        let bottom = MaxF(columnY[0], columnY[1]);
        this.m_status = this.AddText(content, heading, 30, 0.0, bottom + 8.0, 2640.0, 50.0, this.Teal(), true);
        this.m_hints = this.AddText(content, heading, 26, 0.0, bottom + 62.0, 2640.0, 44.0, this.Red(), true);

        this.m_screen = page;
        if IsDefined(this.m_menuRoot) {
            this.m_menuRoot.SetVisible(false);
        }
        this.UpdateScreen();
        NightWheel_Event("wheel-screen-open");
    }

    private func ClearSliders() {
        ArrayClear(this.m_sliderValues);
        ArrayClear(this.m_sliderFills);
        ArrayClear(this.m_sliderControls);
        ArrayClear(this.m_sliderShown);
    }

    // Uses the row from the game's own settings menu, so it looks the same. The row's controller
    // expects a game setting, which these values aren't, so only its look and slider are used, and
    // this script handles its arrows. False means the row couldn't be used, and the page draws its
    // own instead, which also covers a game patch renaming anything here.
    private func AddGameSlider(content: ref<inkCanvas>, index: Int32, x: Float, y: Float) -> Bool {
        if !IsDefined(this.m_menuController) {
            NightWheel_Event("game slider: no menu controller");
            return false;
        }
        let row = this.m_menuController.SpawnFromExternal(content, r"base\\gameplay\\gui\\fullscreen\\settings\\settings_main.inkwidget", n"settingsSelectorInt") as inkCompoundWidget;
        if !IsDefined(row) {
            NightWheel_Event("game slider: row not found");
            return false;
        }
        let label = row.GetWidget(n"layout/labels/label") as inkText;
        let value = row.GetWidget(n"layout/container/slidingArea/knob/txtValue") as inkText;
        let container = row.GetWidget(n"layout/container");
        let left = row.GetWidget(n"layout/container/btnLeft");
        let right = row.GetWidget(n"layout/container/btnRight");
        let control: ref<inkSliderController>;
        if IsDefined(container) {
            control = container.GetController() as inkSliderController;
        }
        if !IsDefined(label) || !IsDefined(value) || !IsDefined(left) || !IsDefined(right) || !IsDefined(control) {
            NightWheel_Event("game slider: row has changed");
            content.RemoveChild(row);
            return false;
        }
        row.SetAnchor(inkEAnchor.TopLeft);
        row.SetMargin(new inkMargin(x, y, 0.0, 0.0));
        row.SetSize(new Vector2(1800.0, 80.0));
        let modified = row.GetWidget(n"modifiedFlag");
        if IsDefined(modified) {
            modified.SetVisible(false);
        }
        let hints = row.GetWidget(n"layout/container/inputHintsContainer");
        if IsDefined(hints) {
            hints.SetVisible(false);
        }
        label.SetText(NightWheel_SliderLabel(index));
        let shown = NightWheel_SliderFraction(index);
        control.Setup(0.0, 1.0, shown, 0.01);
        control.SetInputDisabled(true);
        let name = StringToName(IntToString(index));
        left.SetName(name);
        right.SetName(name);
        left.RegisterToCallback(n"OnRelease", this, n"OnSliderDown");
        right.RegisterToCallback(n"OnRelease", this, n"OnSliderUp");
        let noFill: ref<inkRectangle>;
        ArrayPush(this.m_sliderValues, value);
        ArrayPush(this.m_sliderFills, noFill);
        ArrayPush(this.m_sliderControls, control);
        ArrayPush(this.m_sliderShown, shown);
        NightWheel_Event(s"game slider: row \(index) added");
        return true;
    }

    private func AddSlider(content: ref<inkCanvas>, body: String, index: Int32, x: Float, y: Float) {
        let boxX = x + 640.0;
        let boxW = 620.0;
        let name = StringToName(IntToString(index));
        this.AddText(content, body, 36, x + 20.0, y, 620.0, 68.0, this.Red(), false).SetText(NightWheel_SliderLabel(index));
        let down = this.AddRect(content, boxX, y + 6.0, 70.0, 56.0, this.Red());
        down.SetOpacity(0.35);
        down.SetName(name);
        down.SetInteractive(true);
        down.RegisterToCallback(n"OnRelease", this, n"OnSliderDown");
        this.AddText(content, body, 40, boxX, y, 70.0, 68.0, new HDRColor(1.0, 1.0, 1.0, 1.0), true).SetText("-");
        let up = this.AddRect(content, boxX + boxW - 70.0, y + 6.0, 70.0, 56.0, this.Red());
        up.SetOpacity(0.35);
        up.SetName(name);
        up.SetInteractive(true);
        up.RegisterToCallback(n"OnRelease", this, n"OnSliderUp");
        this.AddText(content, body, 40, boxX + boxW - 70.0, y, 70.0, 68.0, new HDRColor(1.0, 1.0, 1.0, 1.0), true).SetText("+");
        let trackX = boxX + 80.0;
        let trackW = boxW - 160.0;
        let track = this.AddRect(content, trackX, y + 6.0, trackW, 56.0, this.Red());
        track.SetOpacity(0.08);
        let fill = this.AddRect(content, trackX, y + 6.0, trackW, 56.0, this.Teal());
        fill.SetOpacity(0.45);
        let noControl: ref<inkSliderController>;
        ArrayPush(this.m_sliderFills, fill);
        ArrayPush(this.m_sliderControls, noControl);
        ArrayPush(this.m_sliderShown, -1.0);
        ArrayPush(this.m_sliderValues, this.AddText(content, body, 34, trackX, y, trackW, 68.0, new HDRColor(1.0, 1.0, 1.0, 1.0), true));
    }

    protected cb func OnSliderDown(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            NightWheel_SliderAdjust(StringToInt(NameToString(event.GetCurrentTarget().GetName())), -1);
        }
    }

    protected cb func OnSliderUp(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            NightWheel_SliderAdjust(StringToInt(NameToString(event.GetCurrentTarget().GetName())), 1);
        }
    }

    protected cb func OnBoostDown(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            NightWheel_BoostAdjust(-1);
        }
    }

    protected cb func OnBoostUp(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            NightWheel_BoostAdjust(1);
        }
    }

    protected cb func OnRowHover(event: ref<inkPointerEvent>) -> Bool {
        NightWheel_ScreenSelect(StringToInt(NameToString(event.GetCurrentTarget().GetName())));
    }

    protected cb func OnRowRelease(event: ref<inkPointerEvent>) -> Bool {
        if event.IsAction(n"click") {
            NightWheel_Event("wheel-row-clicked");
            NightWheel_ScreenSelect(StringToInt(NameToString(event.GetCurrentTarget().GetName())));
            NightWheel_ScreenCommand(3);
        }
    }

    private func HideScreen() {
        if IsDefined(this.m_screen) {
            NightWheel_ScreenClose();
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
            this.ClearSliders();
            if IsDefined(this.m_menuRoot) {
                this.m_menuRoot.SetVisible(true);
            }
            NightWheel_Event("wheel-screen-closed");
        }
    }

    private func HideAll() {
        this.HideScreen();
        if IsDefined(this.m_refresh) {
            this.m_refresh.Stop();
            this.m_refresh = null;
        }
        if IsDefined(this.m_refreshHost) {
            this.m_refreshHost.SetVisible(false);
            this.m_refreshHost = null;
        }
    }

    private func UpdateScreen() {
        if !IsDefined(this.m_screen) {
            return;
        }
        if !NightWheel_ScreenTick() {
            this.HideScreen();
            return;
        }
        let selected = NightWheel_ScreenSelected();
        let row = 0;
        while row < ArraySize(this.m_rowValues) {
            this.m_rowValues[row].SetText(NightWheel_ScreenRowValue(row));
            if NightWheel_ScreenRowLit(row) {
                this.m_rowMarks[row].SetOpacity(0.7);
            } else {
                this.m_rowMarks[row].SetOpacity(row == selected ? 0.22 : 0.03);
            }
            row += 1;
        }
        let slider = 0;
        while slider < ArraySize(this.m_sliderValues) {
            this.m_sliderValues[slider].SetText(NightWheel_SliderValue(slider));
            let fraction = NightWheel_SliderFraction(slider);
            if IsDefined(this.m_sliderFills[slider]) {
                this.m_sliderFills[slider].SetSize(new Vector2(460.0 * fraction, 56.0));
            }
            // Only when it changes, because this runs on every refresh.
            if IsDefined(this.m_sliderControls[slider]) && NotEquals(fraction, this.m_sliderShown[slider]) {
                this.m_sliderControls[slider].ChangeProgress(fraction);
                this.m_sliderShown[slider] = fraction;
            }
            slider += 1;
        }
        let percent = NightWheel_BoostPercent();
        this.m_boostValue.SetText(s"\(percent)%");
        this.m_boostFill.SetSize(new Vector2(460.0 * Cast<Float>(percent) / 200.0, 56.0));
        this.m_status.SetText(NightWheel_ScreenStatus());
        this.m_hints.SetText(NightWheel_ScreenHints());
    }
}
