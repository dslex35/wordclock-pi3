//! Word Clock driver for the Cyntech / Kiwi Electronics 8x8 WS2812 word clock kit.
//!
//! Hardware: Raspberry Pi (B+, 2, 3 or Zero) driving an 8x8 / 64 LED WS2812 panel
//! on GPIO 18 (PWM0). LED indices below are taken directly from the official
//! Cyntech Python reference: https://github.com/CyntechUK/Wordclock
//!
//! Each letter is given a fresh random color whenever the displayed time
//! changes (i.e. when the set of active letters changes). Colors stay stable
//! between updates so the panel doesn't flicker every second.
//!
//! Run as root (the rpi_ws281x library needs /dev/mem):
//!     sudo ./wordclock-rust-pwm

use chrono::{Local, Timelike};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType};
use std::thread;
use std::time::Duration;

const LED_COUNT: i32 = 64; // 8x8 panel
const GPIO_PIN: i32 = 18; // PWM0 - Cyntech kit wiring
const DMA_CHANNEL: i32 = 10; // 10 is safe on modern Raspberry Pi OS

fn main() {
    println!("Rust Word Clock (Cyntech 8x8 / GPIO 18) starting...");

    let channel = ChannelBuilder::new()
        .pin(GPIO_PIN)
        .count(LED_COUNT)
        .strip_type(StripType::Ws2812)
        .brightness(120) // 0..=255
        .build();

    let mut controller = ControllerBuilder::new()
        .freq(800_000)
        .dma(DMA_CHANNEL)
        .channel(0, channel)
        .build()
        .expect("Failed to create controller - run with sudo!");

    let mut rng = StdRng::from_entropy();

    // Per-LED color buffer. None = LED is off.
    // We regenerate the colors of every active LED whenever the set of active
    // LEDs changes.
    let mut colors: [Option<[u8; 4]>; LED_COUNT as usize] = [None; LED_COUNT as usize];
    let mut last_active: Vec<usize> = Vec::new();

    loop {
        let now = Local::now();
        let mut active = active_indices(now.hour(), now.minute());
        active.sort_unstable();
        active.dedup();

        if active != last_active {
            // Time display has changed -> reroll the color of every active letter.
            colors = [None; LED_COUNT as usize];
            for &idx in &active {
                if idx < LED_COUNT as usize {
                    colors[idx] = Some(random_color(&mut rng));
                }
            }
            last_active = active;
        }

        render(&mut controller, &colors);
        thread::sleep(Duration::from_secs(1));
    }
}

fn render(controller: &mut Controller, colors: &[Option<[u8; 4]>]) {
    let off: [u8; 4] = [0, 0, 0, 0];
    let leds = controller.leds_mut(0);
    for (led, color) in leds.iter_mut().zip(colors.iter()) {
        *led = color.unwrap_or(off);
    }
    controller.render().expect("Render failed");
}

/// Generates a random vivid color. Hue is uniform on [0, 360); saturation and
/// value are pinned to the top of the range so colors are always bright and
/// saturated rather than washed-out or near-black.
fn random_color(rng: &mut impl Rng) -> [u8; 4] {
    let hue = rng.gen_range(0.0_f32..360.0);
    let sat = 1.0_f32;
    let val = 1.0_f32;
    let (r, g, b) = hsv_to_rgb(hue, sat, val);
    // RawColor byte order in this crate for WS2812 is [B, G, R, W].
    [b, g, r, 0]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let h_prime = (h / 60.0) % 6.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match h_prime as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to_u8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to_u8(r1), to_u8(g1), to_u8(b1))
}

/// Returns the set of LED indices that should light up for the given wall-clock time.
/// All ranges follow the Cyntech Python implementation exactly.
fn active_indices(hour: u32, minute: u32) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();

    // Minute words (inclusive 5-minute windows, matching Cyntech's main.py).
    let m = minute as i32;
    if (3..=7).contains(&m) {
        out.extend(MFIVE);
        out.extend(PAST);
    } else if (8..=12).contains(&m) {
        out.extend(MTEN);
        out.extend(PAST);
    } else if (13..=17).contains(&m) {
        out.extend(QUARTER);
        out.extend(PAST);
    } else if (18..=22).contains(&m) {
        out.extend(TWENTY);
        out.extend(PAST);
    } else if (23..=27).contains(&m) {
        out.extend(TWENTY);
        out.extend(MFIVE);
        out.extend(PAST);
    } else if (28..=32).contains(&m) {
        out.extend(HALF);
        out.extend(PAST);
    } else if (33..=37).contains(&m) {
        out.extend(TWENTY);
        out.extend(MFIVE);
        out.extend(TO);
    } else if (38..=42).contains(&m) {
        out.extend(TWENTY);
        out.extend(TO);
    } else if (43..=47).contains(&m) {
        out.extend(QUARTER);
        out.extend(TO);
    } else if (48..=52).contains(&m) {
        out.extend(MTEN);
        out.extend(TO);
    } else if (53..=57).contains(&m) {
        out.extend(MFIVE);
        out.extend(TO);
    }
    // 0..=2 and 58..=59 -> only the hour word (acts as "o'clock").

    // Round hour up after :32, matching the Python reference.
    let mut h = hour;
    if m > 32 {
        h += 1;
    }
    let h12 = match h % 24 {
        0 => 12,
        h if h > 12 => h - 12,
        h => h,
    };

    let hour_word: &[usize] = match h12 {
        1 => ONE,
        2 => TWO,
        3 => THREE,
        4 => FOUR,
        5 => FIVE_H,
        6 => SIX,
        7 => SEVEN,
        8 => EIGHT,
        9 => NINE,
        10 => TEN_H,
        11 => ELEVEN,
        _ => TWELVE,
    };
    out.extend(hour_word);
    out
}

// --- LED index tables (from CyntechUK/Wordclock main.py) -------------------

// Minute words
const MFIVE: &[usize] = &[16, 17, 18, 19];
const MTEN: &[usize] = &[1, 3, 4];
const QUARTER: &[usize] = &[8, 9, 10, 11, 12, 13, 14];
const TWENTY: &[usize] = &[1, 2, 3, 4, 5, 6];
const HALF: &[usize] = &[20, 21, 22, 23];
const PAST: &[usize] = &[25, 26, 27, 28];
const TO: &[usize] = &[28, 29];

// Hour words
const ONE: &[usize] = &[57, 60, 63];
const TWO: &[usize] = &[48, 49, 57];
const THREE: &[usize] = &[43, 44, 45, 46, 47];
const FOUR: &[usize] = &[56, 57, 58, 59];
const FIVE_H: &[usize] = &[32, 33, 34, 35];
const SIX: &[usize] = &[40, 41, 42];
const SEVEN: &[usize] = &[40, 52, 53, 54, 55];
const EIGHT: &[usize] = &[35, 36, 37, 38, 39];
const NINE: &[usize] = &[60, 61, 62, 63];
const TEN_H: &[usize] = &[39, 47, 55];
const ELEVEN: &[usize] = &[50, 51, 52, 53, 54, 55];
const TWELVE: &[usize] = &[48, 49, 50, 51, 53, 54];
