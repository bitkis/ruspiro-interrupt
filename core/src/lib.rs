/***********************************************************************************************************************
 * Copyright (c) 2019 by the authors
 *
 * Author: André Borrmann
 * License: Apache License 2.0
 **********************************************************************************************************************/
#![doc(html_root_url = "https://docs.rs/ruspiro-interrupt-core/0.3.1")]
#![no_std]
#![feature(asm)]

//! # Interrupt Core functions
//!
//! Core functions to enable/disable interrupts globally. This is splitted from the
//! [``ruspiro-interrupt``](https://crates.io/crates/ruspiro-interrupt) crate to remove circular dependencies between
//! the interrupt crate and others (e.g. ``ruspiro-singleton``).

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

// simple state to track whether we are currently running inside an IRQ
// this is usually set and cleared by the interrupt handler [interrupt_handler]
pub static IRQ_HANDLER_ACTIVE: AtomicBool = AtomicBool::new(false);

// last interrupt mask bits before globally disabling interrupts
// TODO: This stores the irq mask globally cross cores. If different cores would like to store
//       a different state this is not reflected at the moment. For the time beeing it is assumed
//       interrupts are only taken on main core and interrupt routing is setup accordingly
pub static IRQ_MASK: AtomicU32 = AtomicU32::new(0xF);

/// Function used to store a cross core global flag that an interrupt is currently
/// handled
pub fn entering_interrupt_handler() {
    IRQ_HANDLER_ACTIVE.store(true, Ordering::Release);
}

/// Function used to clear a cross core global flag that no interrupt is currently
/// handled
pub fn leaving_interrupt_handler() {
    IRQ_HANDLER_ACTIVE.store(false, Ordering::Release);
}

/// globally enabling interrupts (IRQ/FIQ) to be triggered
pub fn enable_interrupts() {
    enable_irq();
    enable_fiq();
}

/// globally disabling interrupts (IRQ/FIQ) from beeing triggered
pub fn disable_interrupts() {
    // in aarch64 mode the interrupts are disabled by default on entering the IRQ exception
    // no need to disable
    #[cfg(target_arch = "aarch64")]
    {
        if IRQ_HANDLER_ACTIVE.load(Ordering::Acquire) {
            return;
        }
    }
    let last_mask = get_interrupt_mask();
    disable_irq();
    disable_fiq();
    let current_mask = get_interrupt_mask();
    // We might disable after we have disabled after an enabled state
    // so just storing the last value might override the beginning enabled state
    // So if the last mask differs from the current one store the last one
    // other wise keep the stored value. 
    if last_mask != current_mask {
        IRQ_MASK.store(last_mask, Ordering::SeqCst);
    }
}

/// globally re-enabling interrupts (IRQ/FIQ) to be triggered. This is done based on the global state
/// that was set before the interrupts were disable using the [``disable_interrupts``] function.
pub fn re_enable_interrupts() {
    // in aarch64 mode the interrupts are disabled by default on entering
    // no need to re-enable when running inside interrupt handler
    #[cfg(target_arch = "aarch64")]
    {
        if IRQ_HANDLER_ACTIVE.load(Ordering::Acquire) {
            return;
        }
    }
    let mask = IRQ_MASK.load(Ordering::SeqCst);
    if (mask & 0x2) == 0 { enable_irq() };
    if (mask & 0x1) == 0 { enable_fiq() };
}

/// globally enable ``IRQ`` interrupts to be triggered
fn enable_irq() {
    #[cfg(target_arch = "arm")]
    unsafe {
        asm!(
            "cpsie i
              isb"
        ) // as per ARM spec the ISB ensures triggering pending interrupts
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "msr daifclr, #2
              isb"
        ) // as per ARM spec the ISB ensures triggering pending interrupts
    };
}

/// globally enable ``FIQ`` interrupts to be triggered
fn enable_fiq() {
    #[cfg(target_arch = "arm")]
    unsafe {
        asm!(
            "cpsie f
              isb"
        ) // as per ARM spec the ISB ensures triggering pending interrupts
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "msr daifclr, #1
              isb"
        ) // as per ARM spec the ISB ensures triggering pending interrupts
    };
}

/// globally disable ``IRQ`` interrupts from beeing triggered. This function stores the state of the current enabling/disabling
/// of interrupts. If ``disable`` is called multiple times after each other this will than ultimately store "disabled" as
/// last state. In this case a previous enabled state (before the multiple calls) is not able to recover with a call to [``re_enable_irq``].
fn disable_irq() {
    // remember the last IRQ state
    //let state = get_interrupt_state();

    #[cfg(target_arch = "arm")]
    unsafe {
        asm!("cpsid i")
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("msr daifset, #2")
    };
}

/// globally disable ``FIQ`` interrupts from beeing triggered. This function stores the state of the current enabling/disabling
/// of interrupts. If ``disable`` is called multiple times after each other this will than ultimately store "disabled" as
/// last state. In this case a previous enabled state (before the multiple calls) is not able to recover with a call to [``re_enable_fiq``].
fn disable_fiq() {
    // remember the last FIQ state
    //let state = get_fault_state();

    #[cfg(target_arch = "arm")]
    unsafe {
        asm!("cpsid f")
    };
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("msr daifset, #1")
    };
}

pub fn get_interrupt_mask() -> u32 {
    #[cfg(target_arch = "arm")]
    unsafe {
        let state: u32;
        asm!("MRS $0, CPSR":"=r"(state):::"volatile");
        // irq enabled if mask bit was not set
        (state >> 6) & 0x3
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        let state: u32;
        asm!("MRS $0, DAIF":"=r"(state):::"volatile");
        // irq enabled if mask bit was not set
        (state >> 6) & 0x3
    }

    #[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
    return 0;
}
