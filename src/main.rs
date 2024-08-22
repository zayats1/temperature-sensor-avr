#![no_std]
#![no_main]

use ag_lcd::{Display, LcdDisplay};
use arduino_hal::{
    clock, default_serial, delay_ms, hal::delay::Delay,
    prelude::_unwrap_infallible_UnwrapInfallible,
};

use numtoa::NumToA;
use onewire::{ds18b20, DeviceSearch, OneWire};

use panic_halt as _;
use ufmt::uwriteln;

const MSG_LEN: usize = 12;

const MALESIGN: [u8; 8] = [
    0b00000, 0b00111, 0b00011, 0b00101, 0b11110, 0b10010, 0b10010, 0b11110,
];

const MALESIGN_IDX: u8 = 0;

const DEGREE_SYM: [u8; 8] = [
    0b00110, 0b01001, 0b00110, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
];

const DEGREE_SYM_IDX: u8 = 1;

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);

    let mut serial = default_serial!(dp, pins, 57600);

    uwriteln!(&mut serial, "Hello from Arduino!\r").unwrap_infallible();

    let mut sensor_pin = pins.a2.into_opendrain();

    let mut wire = OneWire::new(&mut sensor_pin, false);

    // The library supports only onte type, so i need to erase pin numbers
    let en = pins.d9.into_output().downgrade();
    let rs = pins.d8.into_output().downgrade();

    let d4 = pins.d4.into_output().downgrade();
    let d5 = pins.d5.into_output().downgrade();
    let d6 = pins.d6.into_output().downgrade();
    let d7 = pins.d7.into_output().downgrade();

    let lcd_delay = Delay::<clock::MHz16>::new();

    let mut delay = Delay::<clock::MHz16>::new();

    let mut lcd = LcdDisplay::new(rs, en, lcd_delay)
        .with_display(Display::Off)
        .with_half_bus(d4, d5, d6, d7)
        .with_lines(ag_lcd::Lines::TwoLines)
        .with_autoscroll(ag_lcd::AutoScroll::Off)
        .build();

    if wire.reset(&mut delay).is_err() {
        uwriteln!(&mut serial, "ERROR: missing pullup or error on line").unwrap_infallible();
        panic!();
    }

    lcd.display_on();
    lcd.clear();
    lcd.set_character(MALESIGN_IDX, MALESIGN);
    lcd.set_character(DEGREE_SYM_IDX, DEGREE_SYM);
    lcd.home();

    lcd.set_cursor(ag_lcd::Cursor::On);
    lcd.set_position(0, 0);

    lcd.write(MALESIGN_IDX);
    lcd.print("Temperature");
    lcd.write(MALESIGN_IDX);
    lcd.print(" =");
    lcd.set_position(0, 1);

    let mut buffer = [0u8; 16];

    loop {
        let mut search = DeviceSearch::new();
        while let Ok(Some(sensor)) = wire.search_next(&mut search, &mut delay) {
            match sensor.address[0] {
                ds18b20::FAMILY_CODE => {
                    let ds18b20 = ds18b20::DS18B20::new::<()>(sensor).unwrap();

                    // request sensor to measure temperature
                    let resolution = ds18b20.measure_temperature(&mut wire, &mut delay).unwrap();

                    // wait for compeltion, depends on resolution
                    delay_ms(resolution.time_ms().into());

                    // read temperature
                    let temperature = ds18b20.read_temperature(&mut wire, &mut delay).unwrap();

                    let (val, frac) = split_temp(temperature);

                    uwriteln!(&mut serial, "temperature is {}.{}", val, frac).unwrap_infallible();

                    let mut clear_msg = |col: u8, row: u8, msg_len: usize| {
                        lcd.set_position(col, row);
                        for _ in 0..msg_len {
                            lcd.write(' ' as u8);
                        }
                        lcd.set_position(col, row);
                    };

                    clear_msg(0, 1, MSG_LEN);

                    let val = val.numtoa_str(10, &mut buffer);
                    lcd.print(val);
                    lcd.print(".");
                    let frac = frac.numtoa_str(10, &mut buffer);
                    lcd.print(frac);
                    lcd.write(DEGREE_SYM_IDX);
                    lcd.print("C");
                    // let msg_len = val.len() + frac.len() + 3; // triggers borrow checker
                }
                _ => {
                    uwriteln!(&mut serial, " unknown device type").unwrap_infallible();
                }
            }
        }
    }
}

// the code was taken from onewire library
fn split_temp(temperature: u16) -> (i16, i16) {
    if temperature < 0x8000 {
        (temperature as i16 >> 4, (temperature as i16 & 0xF) * 625)
    } else {
        let abs = -(temperature as i16);
        (-(abs >> 4), -625 * (abs & 0xF))
    }
}
