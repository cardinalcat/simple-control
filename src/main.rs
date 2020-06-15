#[macro_use]extern crate serde_derive;
use std::net::{TcpStream, TcpListener};
use args_functional::*;
use std::thread;
use std::io::{Write, Read};
pub mod capture;
use minifb::{Window, WindowOptions};
use capture::*;
use enigo::{Enigo, MouseControllable, KeyboardControllable};
use crosskey::*;
use std::time::Duration;
use opencv::core::*;
use device_query::{DeviceQuery, DeviceState};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeyboardState{
    KeyDown(CrossKey),
    KeyClick(CrossKey),
    KeyUp(CrossKey),
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ButtonMouse {
    Left,
    Middle,
    Right,
}
impl From<&ButtonMouse> for enigo::MouseButton {
    fn from(button: &ButtonMouse) -> enigo::MouseButton {
        match button {
            ButtonMouse::Left => enigo::MouseButton::Left,
            ButtonMouse::Middle => enigo::MouseButton::Middle,
            ButtonMouse::Right => enigo::MouseButton::Right,
        }
    }
}
impl From<&minifb::MouseButton> for ButtonMouse {
    fn from(button: &minifb::MouseButton) -> ButtonMouse {
        match button {
            minifb::MouseButton::Left => ButtonMouse::Left,
            minifb::MouseButton::Middle => ButtonMouse::Middle,
            minifb::MouseButton::Right => ButtonMouse::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MouseState{
    MouseUp(ButtonMouse),
    MouseDown(ButtonMouse),
    MouseClick(ButtonMouse),
    MouseMove((i32, i32)),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceInput {
    Keyboard(KeyboardState),
    Mouse(MouseState)
}

fn run_server(mut stream: TcpStream){
    let mut capturer = CaptureSource::new(SourceType::Screen, 0);
    let mut enigo = Enigo::new();
    println!("starting server");
    stream.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    loop {
        std::thread::sleep(Duration::from_millis(20));
        let mut packet: [u8;65535] = [0;65535];
        let recvlen = match stream.read(&mut packet){
            Ok(len) => len,
            Err(e) => if e.kind() == std::io::ErrorKind::WouldBlock { 0 } else {
                panic!("error occured in recv");
            }
        };
        if recvlen != 0 {
            println!("recv len: {}, packet len: {}", recvlen, packet.to_vec().len());
            let content = String::from_utf8(packet[0..recvlen].to_vec()).unwrap();
            for update in content.split('|'){
                println!("update: {}", update);
                if update == ""{
                    break;
                }
                let input = serde_json::from_str(update).unwrap();
                match input {
                    DeviceInput::Keyboard(keystate) => {
                        match keystate{
                            KeyboardState::KeyDown(key) => enigo.key_down(key.as_enigo_key().unwrap()),
                            KeyboardState::KeyUp(key) => enigo.key_up(key.as_enigo_key().unwrap()),
                            KeyboardState::KeyClick(key) => enigo.key_click(key.as_enigo_key().unwrap()),
                        }
                    },
                    DeviceInput::Mouse(mousestate) => {
                        match mousestate {
                            MouseState::MouseDown(key) => enigo.mouse_down(enigo::MouseButton::from(&key)),
                            MouseState::MouseUp(key) => enigo.mouse_up(enigo::MouseButton::from(&key)),
                            MouseState::MouseClick(key) => enigo.mouse_click(enigo::MouseButton::from(&key)),
                            MouseState::MouseMove((x,y)) => enigo.mouse_move_to(x,y),
                        }
                    }
                }
            }
        }
        let mut frame = capturer.grab_frame().unwrap().to_raw_compressed(".jpg", 15);
        frame.reverse();
        let len = (frame.len() as u32).to_be_bytes();
        frame.extend_from_slice(&len[0..4]);
        frame.reverse();
        stream.write(&frame).unwrap();
        //println!("sent len: {}, frame len: {}", stream.write(&frame).unwrap(), frame.len());
    }
}
pub fn get_mouse_button(window: &minifb::Window) -> Option<minifb::MouseButton> {
    if window.get_mouse_down(minifb::MouseButton::Left){
        return Some(minifb::MouseButton::Left);
    }
    if window.get_mouse_down(minifb::MouseButton::Middle){
        return Some(minifb::MouseButton::Middle);
    }
    if window.get_mouse_down(minifb::MouseButton::Right){
        return Some(minifb::MouseButton::Right);
    }
    None
}
fn run_client(mut stream: TcpStream){
    let mut window = Window::new(
        "Test - ESC to exit",
        1920,
        1080,
        WindowOptions::default(),
    )
    .unwrap_or_else(|e| {
        panic!("{}", e);
    });

    // Limit to max ~60 fps update rate
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    let device_state = DeviceState::new();

    let mut inbuf: [u8;100000] = [0;100000];
    let mut buffer: Vec<u32> = Vec::new();
    println!("about to enter window loop");
    let mut prev_coords = (0,0);
    let mut outbuf = Vec::new();
    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let mouse = device_state.get_mouse();
        let mut recvlen = stream.read(&mut inbuf).unwrap();
        let len: [u8;4] = [inbuf[0],inbuf[1],inbuf[2],inbuf[3]];
        let framelen = u32::from_le_bytes(len);
        while recvlen < (framelen as usize){
            recvlen += stream.read(&mut inbuf[recvlen..100000]).unwrap();
        }
        let matrix = Mat::from_raw_compressed(&inbuf[4..recvlen]);
        matrix.u32frame(&mut buffer);
        window.update_with_buffer(&buffer, 1920, 1080).unwrap();
        inbuf = [0;100000];
        buffer.clear();
        std::mem::drop(matrix);

        let newcoords = mouse.coords;
        if newcoords != prev_coords {
            let pos = DeviceInput::Mouse(MouseState::MouseMove(newcoords));
            outbuf.write(serde_json::to_string(&pos).unwrap().as_bytes()).unwrap();
            outbuf.push(b'|');
            prev_coords = newcoords;
        }
        //match window.get_keys_pressed(KeyRepeat::No) {
          //  Some(keys) => {
                for key in device_state.get_keys() {
                    /*let newcoords = mouse.coords;
                    if newcoords != prev_coords {
                        let pos = DeviceInput::Mouse(MouseState::MouseMove(newcoords));
                        outbuf.write(serde_json::to_string(&pos).unwrap().as_bytes()).unwrap();
                        outbuf.push(b'|');
                        prev_coords = newcoords;
                    }*/
                    let state = DeviceInput::Keyboard(KeyboardState::KeyClick(CrossKey::from(&key)));
                    outbuf.write(serde_json::to_string(&state).unwrap().as_bytes()).unwrap();
                    outbuf.push(b'|');
                }
            //},
            //None => (),
       // }
       
        match get_mouse_button(&window){
            Some(button) => {
                let state = DeviceInput::Mouse(MouseState::MouseClick(ButtonMouse::from(&button)));
                outbuf.write(serde_json::to_string(&state).unwrap().as_bytes()).unwrap();
                outbuf.push(b'|');
                println!("sent packet");
            },
            None => (),
        }
        stream.write(&outbuf).unwrap();
        outbuf.clear();
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn main() {
    let mut args = Arguments::new();

    args.invoke_callback("--connect", move |args,_| {
        let stream = TcpStream::connect(args.get(0).unwrap().get_name()).unwrap();
        //thread::spawn(move || {
            run_client(stream);
        //});
    });
    args.invoke_callback("--bind",move |args, _| {
        let listener = TcpListener::bind(args.get(0).unwrap().get_name()).unwrap();
        for stream in listener.incoming(){
            thread::spawn(move || {
                let stream = stream.unwrap();
                run_server(stream);
            });
        }
    });
    args.parse();
}
