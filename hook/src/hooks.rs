use common::{win, FNativeFuncPtr, UClass, UFunction, UObject};
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr;

mod detour;
use detour::Detour;

mod patch;
use patch::Patch;

mod user;

static mut DRAW_TRANSITION: *const c_void = ptr::null();
static mut ON_ITEM_AMOUNT_CHANGED: MaybeUninit<FNativeFuncPtr> = MaybeUninit::uninit();
static mut GET_ITEM_NAME: MaybeUninit<FNativeFuncPtr> = MaybeUninit::uninit();
static mut ON_FLARE: MaybeUninit<FNativeFuncPtr> = MaybeUninit::uninit();
static mut ON_KEYPRESS_INSERT: MaybeUninit<FNativeFuncPtr> = MaybeUninit::uninit();

static mut AMMO_DRIVEN_WEAPON: *const UClass = ptr::null();
static mut THROWN_GRENADE_ITEM: *const UClass = ptr::null();
static mut DOUBLE_DRILL_ITEM: *const UClass = ptr::null();
static mut HITSCAN_BASE_COMPONENT: *const UClass = ptr::null();

static mut SERVER_REGISTER_HIT: *mut UFunction = ptr::null_mut();
static mut SERVER_REGISTER_HIT_MULTI: *mut UFunction = ptr::null_mut();
static mut SERVER_DAMAGE_TARGET: *mut UFunction = ptr::null_mut();
static mut SERVER_REGISTER_HIT_TERRAIN: *mut UFunction = ptr::null_mut();
static mut SERVER_REGISTER_HIT_DESTRUCTABLE: *mut UFunction = ptr::null_mut();
static mut SERVER_REGISTER_RICOCHET_HIT: *mut UFunction = ptr::null_mut();
static mut SERVER_REGISTER_RICOCHET_HIT_TERRAIN: *mut UFunction = ptr::null_mut();
static mut SERVER_REGISTER_RICOCHET_HIT_DESTRUCTABLE: *mut UFunction = ptr::null_mut();
static mut SERVER_SET_FALL_VELOCITY: *mut UFunction = ptr::null_mut();

#[derive(macros::NoPanicErrorDebug)]
pub enum Error {
    Detour(#[from] detour::Error),
    FindStatic(&'static str),
}

pub struct Hooks {
    _process_remote_function_for_channel: Detour<7>,
    _function_invoke: Detour<5>,
    _add_cheats: Detour<5>,
    _draw_transition: Patch<*const c_void>,
    _on_item_amount_changed: UFunctionHook,
    _get_item_name: UFunctionHook,
    _on_flare: UFunctionHook,
    _on_keypress_insert: UFunctionHook,
}

impl Hooks {
    pub unsafe fn new(module: &win::Module) -> Result<Self, Error> {
        Self::find_statics()?;

        Ok(Self {
            _process_remote_function_for_channel: Detour::new(module, &mut crate::PROCESS_REMOTE_FUNCTION_FOR_CHANNEL, user::my_process_remote_function_for_channel as *const c_void)?,
            _function_invoke: Detour::new(module, &mut crate::FUNCTION_INVOKE, user::my_function_invoke as *const c_void)?,
            _add_cheats: Detour::new(module, &mut crate::ADD_CHEATS, user::my_add_cheats as *const c_void)?,
            _draw_transition: {
                const VTABLE_INDEX: usize = 0x310 / 8;
                let address = (*(*crate::GEngine).GameViewport.cast::<UObject>()).vtable.add(VTABLE_INDEX);
                DRAW_TRANSITION = *address;
                Patch::new(address, user::my_draw_transition as *const c_void)
            },
            _on_item_amount_changed: UFunctionHook::new("Function /Script/FSD.AmmoCountWidget.OnItemAmountChanged", ON_ITEM_AMOUNT_CHANGED.as_mut_ptr(), user::my_on_item_amount_changed)?,
            _get_item_name: UFunctionHook::new("Function /Script/FSD.Item.GetItemName", GET_ITEM_NAME.as_mut_ptr(), user::my_get_item_name)?,
            _on_flare: UFunctionHook::new("Function /Game/Character/Tutorials/Tutorial_Hint_Flares.Tutorial_Hint_Flares_C.OnFlareThrown", ON_FLARE.as_mut_ptr(), user::my_on_flare)?,
            _on_keypress_insert: UFunctionHook::new("Function /Game/Character/BP_PlayerCharacter.BP_PlayerCharacter_C.InpActEvt_Insert_K2Node_InputKeyEvent", ON_KEYPRESS_INSERT.as_mut_ptr(), user::my_on_keypress_insert)?,
        })
    }

    unsafe fn find_statics() -> Result<(), Error> {
        AMMO_DRIVEN_WEAPON = find("Class /Script/FSD.AmmoDrivenWeapon")?.cast();
        THROWN_GRENADE_ITEM = find("Class /Script/FSD.ThrownGrenadeItem")?.cast();
        DOUBLE_DRILL_ITEM = find("Class /Script/FSD.DoubleDrillItem")?.cast();
        HITSCAN_BASE_COMPONENT = find("Class /Script/FSD.HitscanBaseComponent")?.cast();

        SERVER_REGISTER_HIT = find("Function /Script/FSD.HitscanComponent.Server_RegisterHit")?.cast();
        SERVER_REGISTER_HIT_MULTI = find("Function /Script/FSD.MultiHitscanComponent.Server_RegisterHit")?.cast();
        SERVER_REGISTER_HIT_TERRAIN = find("Function /Script/FSD.HitscanComponent.Server_RegisterHit_Terrain")?.cast();
        SERVER_REGISTER_HIT_DESTRUCTABLE = find("Function /Script/FSD.HitscanComponent.Server_RegisterHit_Destructable")?.cast();
        SERVER_REGISTER_RICOCHET_HIT = find("Function /Script/FSD.HitscanComponent.Server_RegisterRicochetHit")?.cast();
        SERVER_REGISTER_RICOCHET_HIT_TERRAIN = find("Function /Script/FSD.HitscanComponent.Server_RegisterRicochetHit_Terrain")?.cast();
        SERVER_REGISTER_RICOCHET_HIT_DESTRUCTABLE = find("Function /Script/FSD.HitscanComponent.Server_RegisterRicochetHit_Destructable")?.cast();
        SERVER_DAMAGE_TARGET = find("Function /Script/FSD.PickaxeItem.Server_DamageTarget")?.cast();
        SERVER_SET_FALL_VELOCITY = find("Function /Script/FSD.FallingStateComponent.Server_SetFallVelocity")?.cast();
        Ok(())
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        unsafe { 
            for &function in user::SEEN_FUNCTIONS.iter() {
                (*function).seen_count = 0;
            }
        }
    }
}

struct UFunctionHook {
    function: *mut UFunction,
    original: FNativeFuncPtr,
}

impl UFunctionHook {
    pub unsafe fn new(f: &'static str, where_to_place_original: *mut FNativeFuncPtr, hook: FNativeFuncPtr) -> Result<UFunctionHook, Error> {
        let function = find(f)?.cast::<UFunction>();
        let original = (*function).Func;
        *where_to_place_original = original;
        (*function).Func = hook;
        Ok(UFunctionHook {
            function,
            original,
        })
    }
}

impl Drop for UFunctionHook {
    fn drop(&mut self) {
        unsafe {
            (*self.function).Func = self.original;
        }
    }
}

unsafe fn find(s: &'static str) -> Result<*mut UObject, Error> {
    (*common::GUObjectArray).find(s).map_err(|_| Error::FindStatic(s))
}