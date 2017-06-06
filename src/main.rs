#![feature(range_contains)]

extern crate config;
extern crate input;
extern crate libc;
extern crate libudev_sys;

use config::{Config, File, FileFormat, Value};
use input::{Libinput, LibinputInterface};
use libc::{c_char, c_int, c_void};
use std::collections::HashMap;
use std::env;
use std::ops::Range;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::{thread, time};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

impl FromStr for Edge {
    type Err = &'static str;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        use Edge::*;

        match string {
            "left"   => Ok(Left),
            "top"    => Ok(Top),
            "right"  => Ok(Right),
            "bottom" => Ok(Bottom),
            _        => Err("invalid edge")
        }
    }
}

#[derive(Clone, Copy)]
struct Gesture {
    start:    (f64, f64),
    position: (f64, f64),
    edge:     Edge,
}

#[derive(Debug)]
struct Trigger {
    command: String,
    edge: Edge,
    position: Range<f64>
}

impl Trigger {
    fn from_hashmap(map: HashMap<String, Value>) -> Result<Trigger, &'static str> {
        let mut map = map;

        fn parse_range_part(val: Option<Value>) -> Result<f64, &'static str> {
            val.ok_or("invalid from or to")?.into_float().ok_or("invalid from or to")
        }

        Ok(Trigger {
            command: map.remove("command").ok_or("no command specified")?
                .into_str().ok_or("invalid command")?,

            edge: Edge::from_str(&map.remove("edge").ok_or("no edge speciifed")?
                                 .into_str().ok_or("invalid edge")?)?,

            position: parse_range_part(map.remove("from"))?
                    ..parse_range_part(map.remove("to"))?,
        })
    }
}

struct Handler {
    touches: Vec<Option<Gesture>>,
    triggers: Vec<Trigger>,
    min_distance: f64,
}

const MAX_TOUCHES: usize = 10;

impl Handler {
    fn touch_down(&mut self, event: input::event::touch::TouchDownEvent) {
        use input::event::touch::*;

        let slot = event.slot().unwrap_or(0) as usize;
        let x    = event.x_transformed(100);
        let y    = event.y_transformed(100);

        if slot >= MAX_TOUCHES { return };

        let threshold: f64     = 1.0;
        let inv_threshold: f64 = 100.0 - threshold;
        let edge = if x < threshold     { Edge::Left   }
              else if y < threshold     { Edge::Top    }
              else if x > inv_threshold { Edge::Right  }
              else if y > inv_threshold { Edge::Bottom }
              else { return };

        let gesture = Gesture {
            start: (x, y),
            position: (x, y),
            edge: edge
        };

        self.touches[slot] = Some(gesture);
    }

    fn touch_motion(&mut self, event: input::event::touch::TouchMotionEvent) {
        use input::event::touch::*;

        let slot = event.slot().unwrap_or(0) as usize;
        let x = event.x_transformed(100);
        let y = event.y_transformed(100);

        if slot >= MAX_TOUCHES { return };

        let gesture = match self.touches[slot].take() {
            Some(gesture) => gesture,
            None          => return
        };

        let gesture = Gesture {
            position: (x, y),
            ..gesture
        };

        self.touches[slot] = Some(gesture);
    }

    fn touch_up(&mut self, event: input::event::touch::TouchUpEvent) {
        use input::event::touch::*;

        let slot = event.slot().unwrap_or(0) as usize;

        if slot >= MAX_TOUCHES { return };

        let gesture = match self.touches[slot].take() {
            Some(gesture) => gesture,
            None          => return
        };

        let distance = match gesture.edge {
            Edge::Left   => gesture.position.0,
            Edge::Top    => gesture.position.1,
            Edge::Right  => 100.0 - gesture.position.0,
            Edge::Bottom => 100.0 - gesture.position.1,
        };

        let position = match gesture.edge {
            Edge::Left | Edge::Right  => gesture.position.1,
            Edge::Top  | Edge::Bottom => gesture.position.0,
        };

        if distance > self.min_distance {
            for trigger in &self.triggers {
                if trigger.edge == gesture.edge && trigger.position.contains(position) {
                    Command::new("sh")
                        .arg("-c")
                        .arg(&trigger.command)
                        .spawn()
                        .expect("failed to execute command");
                }
            }
        }
    }
}

unsafe extern fn open_restricted(path: *const c_char, flags: c_int, _: *mut c_void) -> c_int {
    let fd = libc::open(path, flags);

    if fd < 0 {
        *libc::__errno_location()
    } else {
        fd
    }
}

unsafe extern fn close_restricted(fd: c_int, _: *mut c_void) {
    libc::close(fd);
}

fn main() {
    let mut interface;

    unsafe {
        let udev = libudev_sys::udev_new();

        interface = Libinput::new_from_udev::<()>(LibinputInterface {
            open_restricted: Some(open_restricted),
            close_restricted: Some(close_restricted)
        }, None, udev as *mut c_void);
    }

    let _ = interface.udev_assign_seat("seat0");

    let mut config_path = PathBuf::from(env::var("HOME").unwrap_or(".".to_string()));
    config_path.push(".config/tear/tear.toml");

    let mut config = Config::new();
    config.merge(File::new(config_path.to_str().unwrap(), FileFormat::Toml).required(true));

    let triggers = config.get_array("triggers").expect("no triggers defined");
    let triggers = triggers.into_iter().map(|trigger| {
        match trigger.into_table() {
            Some(trigger) => match Trigger::from_hashmap(trigger) {
                Ok(trigger) => trigger,
                Err(msg) => panic!("invalid trigger: {}", msg)
            },
            None => panic!("triggers defined wrongly")
        }
    }).collect::<Vec<_>>();

    let mut handler = Handler {
        touches: vec![None; MAX_TOUCHES],
        triggers: triggers,
        min_distance: 4.0, // config.get_float("distance").unwrap(),
    };

    loop {
        interface.dispatch().unwrap();

        let if_clone = interface.clone();
        for event in if_clone {
            use input::Event::*;
            use input::event::EventTrait;
            use input::event::DeviceEvent::*;
            use input::event::TouchEvent::*;

            match event {
                Device(Added(ev)) => println!("Handling device {}", ev.device().name()),
                Touch(Down(ev))   => handler.touch_down(ev),
                Touch(Motion(ev)) => handler.touch_motion(ev),
                Touch(Up(ev))     => handler.touch_up(ev),
                _                 => ()
            }
        }

        thread::sleep(time::Duration::from_millis(50));
    }
}
