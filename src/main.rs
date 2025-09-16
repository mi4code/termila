use HUI::*;
use std::{thread, time::Duration};
use std::collections::HashMap;
use std::env;

#[cfg(target_os = "linux")]
use std::os::unix::io::RawFd;
#[cfg(target_os = "linux")]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::ptr;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
#[cfg(target_os = "linux")]
use libc::*;

#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
use std::ptr;
#[cfg(target_os = "windows")]
use windows::core::{PWSTR,PCWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE, GetLastError, STILL_ACTIVE};
//#[cfg(target_os = "windows")]
//use windows::Win32::Security::SECURITY_ATTRIBUTES;
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
#[cfg(target_os = "windows")]
use windows::Win32::System::Console::{ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, HPCON};
#[cfg(target_os = "windows")]
use windows::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, UpdateProcThreadAttribute, PROCESS_INFORMATION, STARTUPINFOEXW, EXTENDED_STARTUPINFO_PRESENT, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, GetExitCodeProcess};


struct OPTIONS {
    shell: String, // your shell or any other command (with or without arguments but no bash operators; if you want bash to create console pauser or pipes or whatever, just use sh -c)
	shell_args: Vec<String>,  // arguments for the shell (if loaded from the config file, args are part of the shell, so just parse them out)
    term: String, // terminal type to be advertised by termila to the shell (possible values: dumb, vt100, xterm, xterm-265color); linux-only
    /*
	ai_url: String, // url of OpenAI API server
    ai_key: String, // OpenAI API key
    ai_model: String, // OpenAI API model
    ai_prompt: String, // OpenAI API system prompt
	*/
    // TODO: color_override: String, // CSS function(s) to modify colors
    // TODO: bell_audio: String, // bell audio file
    // TODO: saved_commands: String, // file with saved commands
	// TODO: saved_history: String, // file with shell history (to allow history modifications)
    // TODO: shell profiles / any shortcuts
}
impl OPTIONS {
	fn new() -> Self { // no config file, default config
		
		// shell, shell_args
		
		let mut _args: Vec<String> = env::args().collect();
		
		let mut shell: String;
		let mut shell_args: Vec<String> = vec![];

		if _args.len() >= 2 {
			shell = _args[1].clone();
			shell_args = _args[2..].to_vec();
		}
		else 
		{
			#[cfg(target_os = "linux")]
			{ shell = "bash".to_string(); /* = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string()); */ }
			#[cfg(target_os = "windows")]
			{ shell = "cmd.exe".to_string(); }
		}
		
		
		// term
		
		let term = std::env::var("TERM").unwrap_or_else(|_| "xterm".to_string());


		return Self {shell, shell_args, term, };
	}
}


struct UI {
	webview: HUI::WebView,
}
impl UI {
	
	fn new() -> Self {
		
		let webview = HUI::WebView::new();
		//webview.hui_tweaks();


        // setup UI
        webview.load_str(r#"<!DOCTYPE html>
        <html>
            <body style="position: relative;">
			
				<!-- TERMINAL SPACE -->
                <p id="console" style="-webkit-user-select: text; margin: 0;  text-wrap: nowrap;"></p>
				
                <button id="ai" style="position: fixed; right: 10px; top: 10px; width: 30px; height: 30px; min-width: unset; padding: unset;">AI</button>
                
				<!-- AUTO-SCROLL BUTTON -->
				<button id="autoscroll-icon" style="position: fixed; right: 10px; bottom: 10px; width: 30px; height: 30px; min-width: unset; z-index: 9; padding: unset;" onclick="this.checked = !this.checked; document.querySelector('#autoscroll-popup').innerHTML = this.checked ? 'auto-scroll disabled' : 'auto-scroll enabled';" checked>&#11015;</button>
				<button id="autoscroll-popup" style="position: fixed; right: 10px; bottom: 10px; width: 30px; height: 30px; min-width: unset; z-index: 8; padding: unset; padding-left: 10px; text-wrap: nowrap; text-align: start;" tabindex="-1" opacity: 0;>auto-scroll enabled</button>
                <style>
					body:has(#autoscroll-icon:hover) #autoscroll-popup {
						width: 200px !important;
						transition: width 2s;
						opacity: 0.7;
					}
				</style>
				
				<div id="dbg" style="display: none; position: fixed; right: 10px; top: 35%; bottom: 35%; width: 30%; height: auto; min-width: unset; padding: 5%; background-color: rgba(50,30,30,0.5)">
					<p>DEBUG</p>
					<br>
					<input type="text" id="buffer">
					<br>
					<button id="submit" onclick="this.checked = true;">submit</button>
					<br>
					<button id="read" onclick="this.checked = true;">read</button>
				</div>
				
				<style>
                    body button:not(#dbg button) {
                        opacity: 0;
                    }
                    body:has(button:hover) button:not(#dbg button):not(#autoscroll-popup){
                        opacity: 0.7;
                    }
                </style>
            </body>
        </html>"#);
        //webview.html_element("body p", "", ""); // HUI bug


        // add keypress callback and event listener + handle copy/paste
        let key_term_handle = webview.call_native( move |args| {
                if let Some(arg) = args.get(0) {
                    if let Ok(val) = arg.parse::<u8>() {
                        //unsafe{CURRENT_PTY.as_ref().unwrap()}.write(val);
						unsafe{CURRENT_PTY.as_mut().unwrap()}.write(val);
                    }
                }
            }, None );
        webview.call_js(&format!("var key_term_handle = {};", key_term_handle), Some(false));
        webview.call_js("
            document.addEventListener('keydown', function(event) {
				
				if (document.activeElement.tagName != 'BODY'){return;} // allow interaction with other inputs too

                if (event.ctrlKey && event.keyCode >= 65 && event.keyCode <= 90) { // ctrl a..z
                    if (event.ctrlKey && event.keyCode >= 67 && window.getSelection().toString() != '') {return;} // ctrl c copy
                    key_term_handle( event.keyCode-64 );
                    // TODO: other non letter characters
                }

                else if (event.keyCode == 38) { // up
                    key_term_handle(27);
                    key_term_handle(91);
                    key_term_handle(65);
                    event.preventDefault();
                }
                else if (event.keyCode == 40) { // down
                    key_term_handle(27);
                    key_term_handle(91);
                    key_term_handle(66);
                    event.preventDefault();
                }
                else if (event.keyCode == 39) { // right
                    key_term_handle(27);
                    key_term_handle(91);
                    key_term_handle(67);
                    event.preventDefault();

                }
                else if (event.keyCode == 37) { // left
                    key_term_handle(27);
                    key_term_handle(91);
                    key_term_handle(68);
                    event.preventDefault();
                }

                else if (event.keyCode == 27) { // esc (also escape character so can be evil)
                    key_term_handle(27);
                }

            });

            document.addEventListener('keypress', function(event) {
				
				if (document.activeElement.tagName != 'BODY'){return;} // allow interaction with other inputs too
				
                key_term_handle(event.keyCode); // TODO: support non-utf8 input, fix windows arrows

            });

            document.addEventListener('paste', function(event) {

                // stop data actually being pasted
                event.stopPropagation();
                event.preventDefault();

                // get data as bytes
                var clipboardData = event.clipboardData || window.clipboardData;
                const encoder = new TextEncoder(); // default is utf-8
                const bytes = encoder.encode(clipboardData.getData('Text'));

                // send bytes to terminal
                bytes.forEach( b => key_term_handle(b) );

            });
        ", Some(false));


        // automatically set terminal size
        webview.call_js(&format!(r#"
            window.addEventListener('resize', () => {{

				console.log("jsresize");

                const span = document.createElement('span');
                span.textContent = 'M';
                span.style.position = 'absolute';
                span.style.visibility = 'hidden';

                document.body.appendChild(span);

                const charWidth = span.offsetWidth;
                const charHeight = span.offsetHeight;

                document.body.removeChild(span);

                const cols = Math.floor(window.innerWidth / charWidth);
                const rows = Math.floor(window.innerHeight / charHeight);

                ({})(cols, rows);

            }});"#,
            webview.call_native( move |args| {

                if let Some(arg) = args.get(0) {
                    if arg.contains(',') {
                        let a = arg.find(',').unwrap();
                        if let Some(cols) = arg.get(..a) {
                            if let Some(rows) = arg.get(a+1..) {
                                if let Ok(c) = cols.parse::<u16>() {
                                    if let Ok(r) = rows.parse::<u16>() {
                                        eprintln!("RESIZE: {}x{}",c,r);
                                        unsafe{CURRENT_PTY.as_mut().unwrap()}.set_size(c,r);
                                    }
                                }
                            }

                        }
                    }
                }

                // TODO: fix HUI rs arguments (the following code is okay, previous is temporary fix)
                /*if let Some(cols) = args.get(0) {
                    if let Some(rows) = args.get(1) {
                        if let Ok(c) = cols.parse::<u16>() {
                            if let Ok(r) = rows.parse::<u16>() {
                                eprintln!("RESIZE: {}x{}",c,r);
                                unsafe{CURRENT_PTY.as_ref().unwrap()}.set_size(c,r);
                            }
                        }
                    }
                }*/

            }, None)
        ), Some(false));



		Self { webview }
    }
	
	
    fn update (&mut self, formated_text: &/*mut*/ Vec<BUFF_formated_text>) {

        // create html
        let mut html = String::new();
        for i in formated_text {
            html.push_str( &format!("<span style=\"{}\">{}</span>", i.style.iter().map(|(key, value)|format!("{}: {};", key, value)).collect::<Vec<String>>().join(" "), i.text.replace(" ", "&nbsp;").replace("\\","\\\\").replace("<","&lt;").replace(">","&gt;").replace("\n","<br>")) );
        }

        // update whole terminal content
        let js_command = format!("document.querySelector('body p').innerHTML=`{}`;", html.replace("`","\\`"));
        self.webview.call_js(&js_command, Some(false));

        // TODO: clear blanks here to sync with html dom, apply clear sequences (segments should have an id paired with dom id, if it gets updated to blank both get deleted here) -- part of partial updates, which it will be possible to implement as soon as the writing/positioning is reliable enough

        // autoscroll
        self.webview.call_js("if (document.querySelector('#autoscroll-icon').checked) {window.scrollTo(0, document.body.scrollHeight);}", Some(false));

    }
	
	
	fn debug_pty_read(&mut self) -> char {
		
		self.webview.call_js("document.querySelector('#dbg').style.display='';", Some(false));
		
		if self.webview.call_js("document.querySelector('#dbg #read').checked;", Some(true)) == "true" {
			self.webview.call_js("document.querySelector('#dbg #read').checked=false;", Some(false));
		
			// TODO: support unicode
			let mut buf = unsafe{CURRENT_PTY.as_mut().unwrap()}.read();
			let mut uni: u8 = 9;
			if buf & 0b1000_0000 == 0 {
				uni = 0;
			}
			else if buf & 0b1110_0000 == 0b1100_0000 {
				uni = 2;
			}
			else if buf & 0b1111_0000 == 0b1110_0000 {
				uni = 3;
			}
			else if buf & 0b1111_1000 == 0b1111_0000 {
				uni = 4;
			}
			else {
				uni = 0;
				buf=35;
			}
			while uni != 0 {
				buf = unsafe{CURRENT_PTY.as_mut().unwrap()}.read();
				if buf != 0 {
					uni-=1;
				}
				if uni == 0 {
					buf=35;
				}
			}
			//let buf = unsafe{CURRENT_PTY.as_mut().unwrap()}.read(); // make the terminal read

			// TODO: escape special codes
			if buf <= 126 && buf >= 32 && buf != 92 {
				self.webview.call_js(&format!("document.querySelector('#dbg input').value+= '{}';", buf as char), Some(false));
			}
			else {
				self.webview.call_js(&format!("document.querySelector('#dbg input').value+= '\\\\x{:02x}';", buf), Some(false));
			}

		}



		if self.webview.call_js("document.querySelector('#dbg #submit').checked;", Some(true)) == "true" {
			self.webview.call_js("document.querySelector('#dbg #submit').checked=false;", Some(false));
			
			return self.webview.call_js(r#"(function() {
												const el = document.querySelector('#dbg #buffer');
												
												if (!el) return 0; // nonexistent

												let val = el.value;
												if (!val) return 0; // empty string

												const firstChar = val[0];
												
												if (firstChar != '\\') { // letter
													el.value = val.slice(1);
													return firstChar.codePointAt(0);
												}
												
												else { // escape
													el.value = val.slice(4);
													return eval("0x"+val.substr(2,2));
												}
												return 0;
					})()"#, Some(true)).parse::<u8>().unwrap_or(0) as char;
		}
		else {
			return '\0';
		}
	
	
	} 
	
	
}


struct BUFF_formated_text<'l> {
    text: String,
    style: HashMap<&'l str, &'l str>, // TODO: use String instead of str to avoid lifetimes and leaks
    updated: bool,
}

struct BUFF<'a> {
    formated_text: Vec<BUFF_formated_text<'a>>, // html_vec [ [<html>,<css or classn>,<q updated>], ] // TODO: ensure that blanks are not preserved
    current_escape: String, // multi-character special commands; contains the sequence from the escape byte to the last character read; if we are not currently reading any sequence (after previous was finished) it is empty string
    current_escape_max_length: usize, // this is to avoid breaking terminal with unsupported/malicious sequences; the value depends on different sequence type
    cursor_position_index: usize,
    cursor_position_character: usize,
    handle_cr_next_time: bool,
}

impl BUFF<'_> {
	
    fn new() -> Option<Self> {
        unsafe {
            Some(Self {
                formated_text: vec![BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:false}, /*BUFF_formated_text{text:"_".to_string(),style:"font-style: bold;".to_string(),updated:false}*/],
                current_escape: "".to_string(),
                current_escape_max_length: 0,
                cursor_position_index: 0,
                cursor_position_character: 1,
                handle_cr_next_time: false,
            })
        }
    }

    

	fn write_buff(&mut self, chr: char) { // write

		// fix invalid cursor position
        if self.cursor_position_index > self.formated_text.len(){
            eprintln!("INVALID POSITION IN BUFFER - RESETING");
            self.cursor_position_index = self.formated_text.len();
            self.cursor_position_character = 0;
            self.formated_text.push( BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:true} );
        }
        if self.cursor_position_character > self.formated_text.get(self.cursor_position_index).unwrap().text.len() {
            eprintln!("INVALID POSITION IN BUFFER - RESETING");
            self.cursor_position_character = self.formated_text.get(self.cursor_position_index).unwrap().text.len();
        }
		
        // remove character at cursor position if overwriting and if its not newline
        let mut index = self.cursor_position_index;
        let mut character = self.cursor_position_character;
		if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\0" { // move to at character position if we are not already
			self.iter_next(& mut index,& mut character);
		}
		
        if /* !self.cursor_position.insert && */ self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") != "\n" {

            let mut start = character/*+1*/;
            let mut end = character+1/*+1*/;

            while ! self.formated_text.get(index).unwrap().text.is_char_boundary(start) && start > 0 {
                start-=1;
            }
            while ! self.formated_text.get(index).unwrap().text.is_char_boundary(end) && end <= self.formated_text.get(index).unwrap().text.len() {
                end+=1;
            }
            if end > self.formated_text.get(index).unwrap().text.len() { end = self.formated_text.get(index).unwrap().text.len(); }

            self.formated_text.get_mut(index).unwrap().text.replace_range(start..end, "");

        }
		

        // place character
        let mut pos = self.cursor_position_character;
        while ! self.formated_text.get(self.cursor_position_index).unwrap().text.is_char_boundary(pos) && pos > 0 {
            pos-=1;
        }
        self.formated_text.get_mut(self.cursor_position_index).unwrap().text.insert(pos, chr);
        self.cursor_position_character += chr.len_utf8();
		
    }



    fn write_raw(&mut self, mut chr: char) {

		if chr == '\x00' {return;} // never accept '\0' for processing - pty implementation returns it when there are no new bytes (it isnt shown anyway and even escape sequences wont contain it)
        //if !chr.is_ascii() {chr='#';} // TODO: accept char (utf8/16/32) or construct it


        if self.current_escape.len() == 0 { // regular text

			// TODO: handle line endings with set_cursor (currently works fine on windows, but not on linux)
            if /*self.formated_text.get(self.cursor_position_index).unwrap().text.chars().nth(self.cursor_position_character-1).unwrap_or(' ') == '\r' ||*/ self.handle_cr_next_time { // carriage return

                self.handle_cr_next_time = false;

                if chr == '\n' || chr == '\x0b' || chr == '\x0c' {
                    // do nothing, the character is useless
                }

                // else if true { self.set_cursor_cr(1,self.get_cursor_r()); }

                else { // move the cursor

                    if self.formated_text.get(self.cursor_position_index).unwrap().text.contains('\n') { // newline in current text style chunk

                        self.cursor_position_character = self.formated_text.get(self.cursor_position_index).unwrap().text.rfind('\n').unwrap()+1;

                    }

                    else { // newline not in current text style chunk

                        self.cursor_position_character = 0; // default position when there is no \n

                        while self.cursor_position_index > 0 {
                            self.cursor_position_index = self.cursor_position_index - 1;
                            if self.formated_text.get(self.cursor_position_index).unwrap().text.contains('\n') {
                                self.cursor_position_character = self.formated_text.get(self.cursor_position_index).unwrap().text.rfind('\n').unwrap()+1;
                                break;
                            }
                        }

                    }

                }
            }
			
            if chr == '\x00' { // null
                // just dont display it
            }
            else if chr == '\x07' { // bell
                eprintln!("BELL !!!");
                // TODO: audio
            }
            else if chr == '\x08' { // backspace
			
				// the cursour should move one character to the left, but its not supposed to delete it
			
				let mut c = self.get_cursor_c();
				if c > 1 { c-=1; }
				self.set_cursor_cr(c,self.get_cursor_r());

                /*if self.cursor_position_character > 0 { // delete previous character
                    self.formated_text.get_mut(self.cursor_position_index).unwrap().text.replace_range(self.cursor_position_character-1..self.cursor_position_character, "");
                    self.cursor_position_character -= 1;
                    self.formated_text.get_mut(self.cursor_position_index).unwrap().updated = true;
                }
                else if self.cursor_position_index > 0 { // find and then delete previous character
                    if let Some(ff) = self.formated_text.iter_mut().skip(self.cursor_position_index).rev().find(|f| f.text.len()>=1) {
                        ff.text.pop();
                        ff.updated = true;
                    }
                    else {
                        eprintln!("CANT BACKSPACE, NO PREVIOUS CHARACTER");
                    }
                }*/

            }
            else if chr == '\n' || chr == '\x0b' || chr == '\x0c' { // newline \n \v \f
			
				// TODO: handle line endings with set_cursor (currently works fine on windows, but not on linux)
			
				/* let mut r = self.get_cursor_r();
				if r == 0 {
					self.set_cursor_cr(99999,0);
					r+=1;
					self.write_buff('\n');
				}
				self.set_cursor_cr(1,r-1); */
			
                if self.get_cursor_r() > 1 {self.set_cursor_cr(1/*self.get_cursor_c()*/,self.get_cursor_r()-1);}
                else {self.write_buff('\n');}
				
            }
            else if chr == '\r' { // carriage return
			
				// TODO: handle line endings with set_cursor (currently works fine on windows, but not on linux)
				// self.set_cursor_cr(1,self.get_cursor_r());
				self.handle_cr_next_time = true;

            }
            else if chr == '\x1b' { // escape chracter
                self.current_escape.push('\x1b'); // start escape sequence
            }
            else { // any other character
                self.write_buff(chr);
            }

        }


        else { // escape sequence
			
            self.current_escape.push(chr);
            self.current_escape_max_length = 3;
			
			
			
			// list of all comon sequences here: https://xtermjs.org/docs/api/vtfeatures/
			
			
			// OSC sequences
			if self.current_escape.starts_with("\x1b]") || self.current_escape.starts_with("\u{9D}") { 

                self.current_escape_max_length = 1024;

				// ending sequence
                if self.current_escape.ends_with("\x07") || self.current_escape.ends_with("\x1b\\") { 
				
					// simplify parsing by removing starting and ending
					let final_escape = &self.current_escape[
						if self.current_escape.starts_with("\x1b]") { 2 } 
						else if self.current_escape.starts_with("\u{9D}") { 2 } // its encoded as two bytes by utf8
						else { 0 }
						..
						self.current_escape.len() 
						- 
						if self.current_escape.ends_with("\x07") { 1 } 
						else if self.current_escape.ends_with("\x1b\\") { 2 } 
						else { 0 }
					];
				
				
					if final_escape.starts_with("0;") { // set title
						// TODO: member called title
						// = &final_escape[2..self.current_escape.len()]
					}
					
					// else if ... // TODO: many more


                    // end sequence
                    self.current_escape = "".to_string();
                }

            }


			// CSI sequences
			if self.current_escape.starts_with("\x1b[") || self.current_escape.starts_with("\u{9B}") {
				
				self.current_escape_max_length = 16;
				
				/*// sequence already ended, testing if there is continuation jammed to it
				if self.current_escape.starts_with("\x1b[") && self.current_escape.contains("\x00") {
					
					if self.current_escape.ends_with("\x00[") { // yes
					
						// make it look like new sequence
						self.current_escape = "\x1b[".to_string();
						
					}
					else { // no
					
						// end sequence
						self.current_escape = "".to_string();
						
						// process current character the correct way
						self.write_raw(by);
						
					}
					
				}*/
				
				// ending sequence
				/*else*/ if (0x40..=0x7E).contains(self.current_escape.as_bytes().last().unwrap()) && self.current_escape.len() > 2 {
					
					// remove starting bytes (always two)
					let final_escape = &self.current_escape[2..];
					
					
					if final_escape.ends_with("m") { // simple color code
						
						// convert escape to css
						fn escape_to_css(sequence: String) -> HashMap<&'static str, &'static str> {

							// strip off the "\x1b[" or "\u{9B}" prefix and the trailing 'm'
							let inner = &sequence[2..sequence.len() - 1];
							
							// init css store
							let mut css: HashMap<&'static str, &'static str> = [].iter().cloned().collect();

							// function to convert 0–255 xterm color code to rgb
							fn xterm256_to_rgb(idx: u8) -> (u8, u8, u8) {
								
								// 0–15: basic ANSI colors
								const BASIC: &[(u8,u8,u8)] = &[
									(0,0,0),       (128,0,0),   (0,128,0),   (128,128,0),
									(0,0,128),     (128,0,128), (0,128,128), (192,192,192),
									(128,128,128), (255,0,0),   (0,255,0),   (255,255,0),
									(0,0,255),     (255,0,255), (0,255,255), (255,255,255),
								];
								if idx < 16 {
									return BASIC[idx as usize];
								}
								
								// 16–231: 6×6×6 color cube
								if idx < 232 {
									let ci = idx - 16;
									let r = ci / 36;
									let g = (ci % 36) / 6;
									let b = ci % 6;
									let level = |n| if n == 0 { 0 } else { 55 + n * 40 };
									return (level(r), level(g), level(b));
								}
								
								// 232–255: grayscale ramp
								let gray = 8 + (idx - 232) * 10;
								return (gray, gray, gray);
								
							}

							// split by ';' to handle multiple codes, no parameters is equivalent to '0'
							let parts: Vec<&str> = if inner.is_empty() {
									vec!["0"]
								} else {
									inner.split(';').collect()
								};

							let mut iter = parts.iter().peekable();

							while let Some(&code) = iter.next() {
								match code {
									
									// Reset
									"0" => {
										css.insert("color","unset");
										css.insert("background-color","unset");
									}
									
									// Foreground standard
									"30" => {css.insert("color","black");}
									"31" => {css.insert("color","red");}
									"32" => {css.insert("color","green");}
									"33" => {css.insert("color","yellow");}
									"34" => {css.insert("color","blue");}
									"35" => {css.insert("color","magenta");}
									"36" => {css.insert("color","cyan");}
									"37" => {css.insert("color","white");}

									// Background standard
									"40" => {css.insert("background-color","black");}
									"41" => {css.insert("background-color","red");}
									"42" => {css.insert("background-color","green");}
									"43" => {css.insert("background-color","yellow");}
									"44" => {css.insert("background-color","blue");}
									"45" => {css.insert("background-color","magenta");}
									"46" => {css.insert("background-color","cyan");}
									"47" => {css.insert("background-color","white");}

									// Foreground bright
									"90" => {css.insert("color","gray");}
									"91" => {css.insert("color","lightcoral");}
									"92" => {css.insert("color","lightgreen");}
									"93" => {css.insert("color","lightyellow");}
									"94" => {css.insert("color","lightskyblue");}
									"95" => {css.insert("color","violet");}
									"96" => {css.insert("color","lightcyan");}
									"97" => {css.insert("color","white");}

									// Background bright
									"100" => {css.insert("background-color","gray");}
									"101" => {css.insert("background-color","lightcoral");}
									"102" => {css.insert("background-color","lightgreen");}
									"103" => {css.insert("background-color","lightyellow");}
									"104" => {css.insert("background-color","lightskyblue");}
									"105" => {css.insert("background-color","violet");}
									"106" => {css.insert("background-color","lightcyan");}
									"107" => {css.insert("background-color","white");}

									// Reset fg/bg
									"39" => {css.insert("color","unset");}
									"49" => {css.insert("background-color","unset");}

									// 256-color fg: 38;5;n
									"38" if iter.peek() == Some(&&"5") => {
										iter.next(); // consume "5"
										if let Some(&n) = iter.next() {
											if let Ok(idx) = n.parse::<u8>() {
												let (r, g, b) = xterm256_to_rgb(idx);
												css.insert("color", Box::leak(format!("rgb({},{},{})", r, g, b).into_boxed_str()));
											}
										}
									}
									
									// 256-color bg: 48;5;n
									"48" if iter.peek() == Some(&&"5") => {
										iter.next();
										if let Some(&n) = iter.next() {
											if let Ok(idx) = n.parse::<u8>() {
												let (r, g, b) = xterm256_to_rgb(idx);
												css.insert("background-color", Box::leak(format!("rgb({},{},{})", r, g, b).into_boxed_str()));
											}
										}
									}
									
									// TrueColor fg: 38;2;R;G;B
									"38" if iter.peek() == Some(&&"2") => {
										iter.next(); // consume "2"
										let r = iter.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
										let g = iter.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
										let b = iter.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
										css.insert("color", Box::leak(format!("rgb({},{},{})", r, g, b).into_boxed_str()));
									}
									
									// TrueColor bg: 48;2;R;G;B
									"48" if iter.peek() == Some(&&"2") => {
										iter.next();
										let r = iter.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
										let g = iter.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
										let b = iter.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
										css.insert("background-color", Box::leak(format!("rgb({},{},{})", r, g, b).into_boxed_str()));
									}

									_ => {
										// unknown or unsupported code, ignore
									}
								}
							}

							return css;
						}
						let css = escape_to_css(self.current_escape.clone());

						// if there is empty style field at current cursor position, update its css 
						if self.cursor_position_index < self.formated_text.len() && self.formated_text.get(self.cursor_position_index).unwrap().text.len() == 0 {
							self.formated_text.get_mut(self.cursor_position_index).unwrap().style.extend(css);
							self.formated_text.get_mut(self.cursor_position_index).unwrap().updated = true;
						}
						// create new style field and switch to it
						else {
							self.formated_text.insert(self.cursor_position_index+1, BUFF_formated_text{text:"".to_string(),style:self.formated_text.get(self.cursor_position_index).unwrap().style.clone(),updated:true});
							self.cursor_position_index += 1;
							self.cursor_position_character = 0;
							self.formated_text.get_mut(self.cursor_position_index).unwrap().style.extend(css);
						}

					}
					
					else if final_escape.ends_with("t") { // set window state
						// TODO: ignore or print it
					}

					else if final_escape.ends_with("r") { // scrolling region
						// TODO
					}

					else if final_escape.ends_with("l") || final_escape.ends_with("h") { // enable or disable features
						
						// while linux used sequences contain ?, windows use non standart format without it (meaning should be the same) - ie. '\x1b[?{number}h/l' or '\x1b[{number}h/l'
						let feature_id = final_escape[if final_escape.starts_with("?"){1}else{0} .. final_escape.len()-1].parse::<u8>().unwrap_or(0);
					
						// TODO: actaually support them

					}
					
					else if final_escape.ends_with("J") { // clear sequences
						
						// self.current_escape == "\x1b[0J" || self.current_escape == "\x1b[1J" || self.current_escape == "\x1b[2J" || self.current_escape == "\x1b[3J"
						// TODO: there are actually some differences between these 
						
						// TODO: this would definitely break partial updates
						self.formated_text = vec![BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:true},];
						self.cursor_position_index = 0;
						self.cursor_position_character = 1;

					}
					
					else if final_escape.ends_with("K") { // clear line
						
						// get cursor position
						let c = self.get_cursor_c();
						let r = self.get_cursor_r();
						
						if final_escape == "2K" { // entire line
							
							// go to beginning of the line
							self.set_cursor_cr(1,r);
							
							// write spaces
							for _ in 0..unsafe{CURRENT_PTY.as_ref().unwrap()}.columns { self.write_buff(' '); }
							
						}
						else if final_escape == "1K" { // from beginning to cursor
							
							// go to beginning of the line
							self.set_cursor_cr(1,r);
							
							// write spaces
							for _ in 0..c-1 { self.write_buff(' '); }
							
						}
						else if final_escape == "0K" || final_escape == "K" { // from cursor to end of line
							
							// write spaces
							for _ in 0..(unsafe{CURRENT_PTY.as_ref().unwrap()}.columns+1).saturating_sub(c) { self.write_buff(' '); }
							
						}
						
						// restore cursor position
						self.set_cursor_cr(c,r);

						// end sequence
						self.current_escape = "".to_string();
					}
			
					else if final_escape.ends_with("X") { // erase in line without moving cursor
					
						// get cursor position
						let c = self.get_cursor_c();
						let r = self.get_cursor_r();
						
						// write spaces
						for _ in 0..final_escape[0..final_escape.len()-1].parse::<u8>().unwrap_or(1) { self.write_buff(' '); }
						
						// restore cursor position
						self.set_cursor_cr(c,r);

						// end sequence
						self.current_escape = "".to_string();
						
					}
					
					else if final_escape.ends_with("H") || final_escape.ends_with("f") { // absolute cursor position
						
						// both coordinates given
						if self.current_escape.contains(";") {
							let r = self.current_escape[2..self.current_escape.find(';').unwrap()].parse::<usize>().unwrap_or(1);
							let c = self.current_escape[self.current_escape.find(';').unwrap()+1..self.current_escape.len()-1].parse::<usize>().unwrap_or(1);
							self.set_cursor_cr(c,unsafe{CURRENT_PTY.as_ref().unwrap()}.rows.saturating_sub(r)+1);
						}
						
						// column ommited so default
						else {
							let r = self.current_escape[2..self.current_escape.find('H').unwrap()].parse::<usize>().unwrap_or(1);
							let c = 1;
							self.set_cursor_cr(c,unsafe{CURRENT_PTY.as_ref().unwrap()}.rows.saturating_sub(r)+1);
						}
						
					}
					
					else if final_escape.ends_with("A") { // cursor up
						let n = self.current_escape[2..self.current_escape.len()-1].parse::<usize>().unwrap_or(1);
						self.set_cursor_cr(self.get_cursor_c(),self.get_cursor_r()+n);
					}
					
					else if final_escape.ends_with("B") { // cursor down
						let n = self.current_escape[2..self.current_escape.len()-1].parse::<usize>().unwrap_or(1);
						let r = self.get_cursor_r();
						if n <= r { self.set_cursor_cr(self.get_cursor_c(),r-n); }
						else { self.set_cursor_cr(self.get_cursor_c(),0); }
					}
					
					else if final_escape.ends_with("C") { // cursor right
						let n = self.current_escape[2..self.current_escape.len()-1].parse::<usize>().unwrap_or(1);
						self.set_cursor_cr(self.get_cursor_c()+n,self.get_cursor_r());
					}
					
					else if final_escape.ends_with("D") { // cursor left
						let n = self.current_escape[2..self.current_escape.len()-1].parse::<usize>().unwrap_or(1);
						let c = self.get_cursor_c();
						if n < c { self.set_cursor_cr(c-n,self.get_cursor_r()); }
						else { self.set_cursor_cr(1,self.get_cursor_r()); }
					}
					
					// else if ... // TODO: some more
					
					
					// pre-end sequence
                    //self.current_escape.push('\x00');
					self.current_escape = "".to_string();
				}
				
			} 
			


            // enforce max length
            if self.current_escape.len() >= self.current_escape_max_length {
                eprintln!("UNKNOWN ESCAPE SEQUENCE: '{}'", self.current_escape);

                // print it to terminal
                let escape = self.current_escape.get(1..).unwrap().to_owned();
                for c in escape.chars() {
                    //self.write_buff(c);
                }
				
				// end sequence
                self.current_escape = "".to_string();
            }

        }


    }



	fn dom_mut(&mut self) -> &/*mut*/ Vec<BUFF_formated_text> {
        &mut self.formated_text
    }



	fn iter_next (&self, index: &mut usize, character: &mut usize) -> bool {
		let this = &self.formated_text;
		
		// same style chunk
		*character += 1;
		while ! this.get(*index).unwrap().text.is_char_boundary(*character) && *character < this.get(*index).unwrap().text.len() {
			*character += 1;
		}
		if *character < this.get(*index).unwrap().text.len() {
			return true;
		}
		
		// next style chunk (with at least one char)
		else { 
			while true {
				if ! (*index+1 < this.len()) {
					return false;
				}
				*index += 1;
				if this.get(*index).unwrap().text.len() > 0 {break;}
			}
			*character = 0;
			return true;
		}
		
	}
	
	fn iter_prev (&self, index: &mut usize, character: &mut usize) -> bool {
		let this = &self.formated_text;
		
		// prev style chunk (with at least one char)
		if *character == 0 { 
			while true {
				if ! (*index > 0) {
					return false;
				}
				*index -= 1;
				if this.get(*index).unwrap().text.len() > 0 {break;}
			}
			*character = this.get(*index).unwrap().text.len()-1;
			while ! this.get(*index).unwrap().text.is_char_boundary(*character) && *character > 0 {
				*character -= 1;
			}
			return true;
		}
		
		// same style chunk
		else {
			*character -= 1;
			while ! this.get(*index).unwrap().text.is_char_boundary(*character) && *character > 0 {
				*character -= 1;
			}
			return true;
		}
		
	}
	
	/*fn iter_valid (&self, index: &mut usize, character: &mut usize) { // TODO: use it or remove it - we expect all positions to be valid
		let this = &self.formated_text;
		if *index >= this.len(){
			eprintln!("INVALID POSITION IN BUFFER - RESETING");
			*index = this.len()-1;
			*character = this.get(*index).unwrap().text.len()-1;
		}
		if *character >= this.get(*index).unwrap().text.len() {
			eprintln!("INVALID POSITION IN BUFFER - RESETING");
			*character = this.get(*index).unwrap().text.len()-1;

			if this.get(*index).unwrap().text.len() == 0 {
				*character=0;
				eprintln!(" HANDLED USIZE UNDERFLOW");
				//return false;
			}
		}
	}*/


    fn set_cursor(&mut self, mut index: usize, mut character: usize) {
		// this function expects index to be < .len() and that character to be .is_char_boundary() && < .len()
		// is and should be used only by set_cursor_cr

        // insert first part to index+1
        self.formated_text.insert(index+1, BUFF_formated_text{text:self.formated_text.get(index).unwrap().text.get(..character).unwrap_or("<TERMILA_PARSER_ERROR>").to_string(),style:self.formated_text.get(index).unwrap().style.clone(),updated:true});
        // insert new patr to index+2
        self.formated_text.insert(index+2, BUFF_formated_text{text:"".to_string(),style:self.formated_text.get(self.cursor_position_index).unwrap().style.clone(),updated:true});
        // insert second part to index+3
            self.formated_text.insert(index+3, BUFF_formated_text{text:self.formated_text.get(index).unwrap().text.get(character..).unwrap_or("<TERMILA_PARSER_ERROR>").to_string(),style:self.formated_text.get(index).unwrap().style.clone(),updated:true});
        // remove part at index
        self.formated_text.remove(index);

        self.cursor_position_index = index+1;
        self.cursor_position_character = 0;

    }

    fn get_cursor_c(&self) -> usize {

        let mut column = 0;

        let mut prev = true;
        let mut index = self.cursor_position_index;
        let mut character = self.cursor_position_character;
        // iter_valid(& self.formated_text,& mut index,& mut character);
        while prev {

            column += 1;

            prev = self.iter_prev(& mut index,& mut character);
			
			if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\n" {break;}

        }

        return column;

    }

    fn get_cursor_r(&self) -> usize {

        let mut row = 0;

        let mut next = true;
        let mut index = self.cursor_position_index;
        let mut character = self.cursor_position_character;
        // iter_valid(& self.formated_text,& mut index,& mut character);
        while next {

            if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\n" {
                row += 1;
            }

            next = self.iter_next(& mut index,& mut character);

        }

        return row;

    }

    fn set_cursor_cr(&mut self, mut column: usize, mut row: usize) {
		
		// store debug statistics
        let old_col = self.get_cursor_c();
        let old_row = self.get_cursor_r();
        let des_col = column;
        let des_row = row;


		// limit values to terminal size
		if column == 0 {column=1;}
        if column >= unsafe{CURRENT_PTY.as_ref().unwrap()}.columns {column=unsafe{CURRENT_PTY.as_ref().unwrap()}.columns;}
        if row >= unsafe{CURRENT_PTY.as_ref().unwrap()}.rows {row=unsafe{CURRENT_PTY.as_ref().unwrap()}.rows;}
		

		// add rows in case there are less of them than the requested move
		// TODO: this is just workaround - allows for more tuis to display correctly but sometimes moves stuff completely elsewhere (consider adding newlines at i0c0 or having them there from start and hide them by the dom generator or scrollback control) 
		let mut index = 0;
		let mut character = 0;
		let mut existing_rows = 1;
		let mut iter = true;
        while iter {
			if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\n" {existing_rows+=1;}
            iter = self.iter_next(& mut index,& mut character);
			if existing_rows > unsafe{CURRENT_PTY.as_ref().unwrap()}.rows {break;}
        }
		// add newlines if needed
		if existing_rows < row+1 {
			self.set_cursor( self.formated_text.len()-1, self.formated_text.get(self.formated_text.len()-1).unwrap().text.len() ); // since we are in set_cursor we can move the cursor freely
			for _ in 0..(unsafe{CURRENT_PTY.as_ref().unwrap()}.rows-existing_rows) { self.write_buff('\n'); }
		}
		
		
		// start at the end
        index = self.formated_text.len()-1;
        character = self.formated_text.get(index).unwrap().text.len();
        //iter_valid(& self.formated_text,& mut index,& mut character);


        // set to begining of given row
        let mut prev = true;
        while prev {

            if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\n" {
				if row == 0 {break;}
                row -= 1;
            }

            prev = self.iter_prev(& mut index,& mut character);

        }


        // set to given column if possible
        let mut next = true;
        while next {

            next = self.iter_next(& mut index,& mut character);

            column -= 1;
            if column == 0 {break;}

            if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\n" {break;}

        }


		// finally set cursor to calculated position
        self.set_cursor(index, character);
		
		
		// add spaces to reach desired column outside existing text
		self.formated_text.insert(self.cursor_position_index+1, BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:true}); // use neutral color
		self.cursor_position_index += 1;
		self.cursor_position_character = 0;
		for _ in 0..column { self.write_buff(' '); } // add spaces without style
		self.formated_text.insert(self.cursor_position_index+1, BUFF_formated_text{text:"".to_string(),style:self.formated_text.get(self.cursor_position_index-1).unwrap().style.clone(),updated:true}); // restore previous color
		self.cursor_position_index += 1;
		self.cursor_position_character = 0;


		// print debug statistics
        eprintln!("SET CURSOR POSITION (c{},r{}) -> (c{},r{}) -> (c{},r{}) = old->desired->final", old_col,old_row, des_col,des_row, self.get_cursor_c(),self.get_cursor_r() );

    }


	/*
	positioning specs:

		terminal specification:
			column: left to right, starts at 1, values over size are interpreted as max
			row: top to bottom, starts at 1, values over size are interpreted as max
			
		set position methods:
			column: (same as terminal specification)
			row: bottom to top, starts at 0, values over size are interpreted as max (termsize-xtermpos, NOT termsize-xtermpos+1) - code: absolute position csi, escape up/down A/B, \r, \n

		dom structure:
			index: index in vector of styled text chunks, should always point to existing one
			character: index where next character will go - either existing position or len value (note: this is u8 byte index not nth char index)
			(cursor position is the character to be overwriten or nul char if at he end of char array)

	*/

}


#[cfg(target_os = "linux")]
struct PTY {
    master: RawFd,
    slave: RawFd,

    rows: usize,
    columns: usize, 
}
#[cfg(target_os = "linux")]
impl PTY {
    fn new (shell: String, shell_args: Vec<String>, term: String) -> Option<Self> {
        unsafe {
            // open PTY master device (using BSD-style management)
            let master = posix_openpt(O_RDWR | O_NOCTTY);

            // needs to be called so slave can be opened
            if master == -1 {
                eprintln!("ERROR: posix_openpt()");
                return None;
            }
            if grantpt(master) == -1 {
                eprintln!("ERROR: grantpt()");
                return None;
            }
            if unlockpt(master) == -1 {
                eprintln!("ERROR: unlockpt()");
                return None;
            }

            // get slave's file descriptor (for our shell subprocess)
            let slave_name = ptsname(master);
            if slave_name == std::ptr::null_mut() {
                eprintln!("ERROR: ptsname()");
                return None;
            }
            let slave_fd = open(slave_name, O_RDWR | O_NOCTTY);
            if slave_fd == -1 {
                eprintln!("ERROR: open()");
                return None;
            }

            // launch
            let pid = fork();
            if pid < 0 {
                eprintln!("ERROR: fork()");
                return None;
            }
            if pid == 0 {
                close(master);

                // create a new session and make it controlling terminal for this process
                setsid();
                if ioctl(slave_fd, TIOCSCTTY, 0) == -1 {
                    eprintln!("ERROR: ioctl(TIOCSCTTY)");
                    return None;
                }

                dup2(slave_fd, STDIN_FILENO);
                dup2(slave_fd, STDOUT_FILENO);
                dup2(slave_fd, STDERR_FILENO);
                close(slave_fd);

                Command::new( shell )
                    .env("TERM", term )
                    .args( shell_args )
                    .exec();
                std::process::exit(1); // return false;
            }
            else { // pid < 0
                close(slave_fd);
                //return true;
            }

            Some(Self { master: master, slave: slave_fd, rows: 99999, columns: 99999})
        }
    }

    fn set_size(&mut self, r: u16, c: u16) -> bool {
        self.rows = r as usize;
        self.columns = c as usize;

        let ws = winsize {
            ws_row: r,
            ws_col: c,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        unsafe {
            // this is the very same ioctl that normal programs use to query the window size (so in theory normal programs can also set the size)
            if ioctl(self.master, TIOCSWINSZ, &ws as *const _ as *const _) >= 0 {
                eprintln!("ERROR: ioctl(TIOCSWINSZ)");
                return false;
            }
            return true;
        }
    }

    fn write(&mut self, b: u8) -> bool {
        unsafe {
            write(self.master, &[b] as *const _ as *const _, 1);
        }
        return true;
    }

    fn read(&mut self) -> u8 {

        let mut readfds: fd_set = unsafe { std::mem::zeroed() };
        unsafe {
            FD_ZERO(&mut readfds);
            FD_SET(self.master, &mut readfds);
        }

        let mut timeout = timeval { tv_sec: 0, tv_usec: 0 };
        let ready = unsafe { select(self.master + 1, &mut readfds, ptr::null_mut(), ptr::null_mut(), &mut timeout) };
        if ready < 0 {
            eprintln!("ERROR: select()");
        }

        if unsafe { FD_ISSET(self.master, &readfds) } {
            let mut buf = [0u8; 1];
            let n = unsafe { read(self.master, buf.as_mut_ptr() as *mut _, 1) };
            if n <= 0 {
                eprintln!("EXIT: nothing to read or error");
                std::process::exit(1);
                //return 255; // unused by utf8, here means exit
            }

            eprintln!("TERMINAL: '{}' {}", buf[0] as char, buf[0]);
            return buf[0];

        }

        // there are no new bytes (better would be to run this in other thread and wait for new bytes - pass nullptr instead of timeout to select)
        // eprintln!("ERROR: terminal read"); // avoid spaming console
        return 0;
    }

}


#[cfg(target_os = "windows")]
struct PTY {
    in_write: HANDLE, // we write to this (goes into the conpty)
    out_read: HANDLE, // we read from this (comes out of the conpty)
    hpc: HPCON, // pseudo console handle
    pi: PROCESS_INFORMATION,
	
	rows: usize,
    columns: usize,
}
#[cfg(target_os = "windows")]
impl PTY {
    fn new (shell: String, shell_args: Vec<String>, term: String) -> Option<Self> {
        unsafe {
			
			// create input and output pipes
			
            // security attributes so pipes are inheritable (not needed, kept just in case)
            /*let mut sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: ptr::null_mut(),
                bInheritHandle: true.into(),
            };*/

            // handles for our pipes
            let mut in_read = HANDLE::default();
            let mut in_write = HANDLE::default();
            let mut out_read = HANDLE::default();
            let mut out_write = HANDLE::default();

            // create pipes
            if CreatePipe(&mut in_read, &mut in_write, None/*Some(&mut sa)*/, 0).is_err() {
                eprintln!("ERROR: CreatePipe(in)");
                return None;
            }
            if CreatePipe(&mut out_read, &mut out_write, None/*Some(&mut sa)*/, 0).is_err() {
                eprintln!("ERROR: CreatePipe(out)");
                return None;
            }
			
			// create console

            // initial size
            let size = windows::Win32::System::Console::COORD { X: 999, Y: 999 };
			
			// create console
			let hpc = match unsafe { CreatePseudoConsole(size, in_read, out_write, 0) } {
				Ok(h) => h,
				Err(err) => {
					eprintln!("ERROR: CreatePseudoConsole() - {:?}", err);
					return None;
				}
			};

            // these handles were cloned by the conpty and we dont need them anymore
            CloseHandle(in_read);
            CloseHandle(out_write);

            // add child process to the pseudo console
			
            // figure out attribute list size
            let mut bytes: usize = 0;
            let mut si_ex: STARTUPINFOEXW = std::mem::zeroed();
            InitializeProcThreadAttributeList( windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST( ptr::null_mut() ), 1, 0, &mut bytes as *mut usize );

            // allocate and init attribute list
			let heap = windows::Win32::System::Memory::GetProcessHeap().unwrap();
			si_ex.lpAttributeList = windows::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST(
				windows::Win32::System::Memory::HeapAlloc(
					heap,
					windows::Win32::System::Memory::HEAP_FLAGS(0),
					bytes
				) as *mut _
			);
			
            if si_ex.lpAttributeList.0.is_null() {
                eprintln!("ERROR: HeapAlloc(attrlist)");
                return None;
            }
            if InitializeProcThreadAttributeList(si_ex.lpAttributeList, 1, 0, &mut bytes).is_err() {
                eprintln!("ERROR: InitializeProcThreadAttributeList()");
                return None;
            }

            // attach the HPCON (pseudo console handle)
            if UpdateProcThreadAttribute(
                si_ex.lpAttributeList,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some(hpc.0 as *mut c_void),
                std::mem::size_of::<HPCON>(),
                None,
                None,
            ).is_err() {
                eprintln!("ERROR: UpdateProcThreadAttribute(PSEUDOCONSOLE)");
                return None;
            }

            si_ex.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

            // launch the shell process
            let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
			let mut cmdline = format!("\"{}\" \"{}\"", shell, shell_args.join("\" \""));
            let mut cmd_w: Vec<u16> = cmdline .encode_utf16().chain(std::iter::once(0)).collect();
            if CreateProcessW(
                PCWSTR::null(),
                PWSTR(cmd_w.as_mut_ptr()),
                None,
                None,
                false, // inherit handles
                EXTENDED_STARTUPINFO_PRESENT,
                None,
                PCWSTR(ptr::null()),
                &mut si_ex.StartupInfo,
                &mut pi,
            ).is_err() {
				let err = GetLastError().0;
				eprintln!("ERROR: CreateProcessW() failed with code {}", err);
                return None;
            }

            // free attribute list (no longer needed)
            DeleteProcThreadAttributeList(si_ex.lpAttributeList);
            windows::Win32::System::Memory::HeapFree( heap, windows::Win32::System::Memory::HEAP_FLAGS(0), Some(si_ex.lpAttributeList.0 as *mut _) );
			
            Some( Self{ in_write, out_read, hpc, pi, rows: 999, columns: 999, })
        }
    }

	fn set_size(&mut self, r: u16, c: u16) -> bool {
        self.rows = r as usize;
        self.columns = c as usize;
		
		let size = windows::Win32::System::Console::COORD {
			X: c as i16,
			Y: r as i16,
		};
		let hr = unsafe{ResizePseudoConsole(self.hpc, size)};
		if hr.is_err() {eprintln!("ERROR: ResizePseudoConsole");}
		hr.is_ok()
	}

    fn write(&mut self, b: u8) -> bool {
        unsafe {
            //let mut written = 0u32;
            !WriteFile(
                self.in_write,
                Some(&[b]), // data to be written
                None, //Some(&mut written as *mut u32), // number of bytes written (optional, not needed)
                None,
            )
            .is_err()
        }
    }

    fn read(&mut self) -> u8 {
        unsafe {
			
			// check if the child process is still running
			let mut code = 0u32;
            if GetExitCodeProcess(self.pi.hProcess, &mut code).is_ok() {
                if code != STILL_ACTIVE.0 as u32 {
					eprintln!("EXIT: child process exited");
					
					// clean exit (code previously in Drop)
					unsafe {
						CloseHandle(self.in_write);
						CloseHandle(self.out_read);
						if self.pi.hProcess.0 != ptr::null_mut() {
							CloseHandle(self.pi.hProcess);
						}
						if self.pi.hThread.0 != ptr::null_mut() {
							CloseHandle(self.pi.hThread);
						}
						if self.hpc.0 != 0 {
							ClosePseudoConsole(self.hpc);
						}
					}
					
					std::process::exit(1);
					//return 255; // unused by utf8, here means exit
				} 
            }
			
            // check if there’s a byte available (non-blocking)
            let mut avail = 0u32;
            let ok = !PeekNamedPipe(
                self.out_read,
                None,
                0,
                None,
                Some(&mut avail),
                None,
            ).is_err();
            if !ok {
                return 0; // other error occured
            }
			if avail == 0 {
				return 0; // there were no new bytes
			}

			// read from the console
            let mut buf = [0u8; 1];
            let mut read = 0u32;
            if ReadFile(
                self.out_read,
                Some(&mut buf), // output buffer
                Some(&mut read), // number of bytes read
                None,
				).is_ok() && read == 1 {
					eprintln!("TERMINAL: '{}' {}", buf[0] as char, buf[0]);
					return buf[0]; // there were bytes to read
            }
			
            return 0; // other error occured
			
        }
    }
	
}


fn read_char<F>(mut read: F) -> char  where F: FnMut() -> u8 {

    // read the first byte
	let mut buf = Vec::new();
    buf.push(read());

    // determine how many bytes we need
    let needed = match buf[0] {
        0x00..=0x7F => 1,  // ASCII
        0xC0..=0xDF => 2,  // 2-byte sequence
        0xE0..=0xEF => 3,  // 3-byte sequence
        0xF0..=0xF7 => 4,  // 4-byte sequence
        _ => 0,            // invalid leading byte
    };

    // read more bytes if needed
    while buf.len() < needed {
        buf.push(read());
    }

    // try to decode
    match str::from_utf8(&buf) {
        Ok(s) => s.chars().next().unwrap_or(' '),
        Err(_) => ' ',
    }
	
}


// unsafe (because of threads)
static mut CURRENT_PTY: Option<PTY> = None;

fn main() {
	
	// load options
	let options = OPTIONS::new();


    // setup terminal
    let pty = match PTY::new(options.shell, options.shell_args, options.term) {
        Some(pty) => pty,
        None => {
            eprintln!("Failed to create PTY");
            return;
        }
    };

    unsafe { CURRENT_PTY = Some(pty); }

    unsafe{CURRENT_PTY.as_mut().unwrap()}.set_size(40,190);


    // init termin processor
    let mut buff = match BUFF::new() {
        Some(buff) => buff,
        None => {
            eprintln!("Failed to create BUFF");
            return;
        }
    };


    // init UI
	let mut ui = UI::new();


    // run main loop

    let mut to_update = 0; // keep count of bytes that were outputed to the terminal but were not yet shown in the ui

    loop {
		
		// TODO: split stdin/stderr/stdout - actually this may exist only in the pty part, buff we will have arg, not separatte method = step one is to put in/err together in main -- best will be to glue stdout/err together in main and we can optionaly send escape sequences between them (for colors or custom ones for detection and styling)


        //let buf = unsafe{CURRENT_PTY.as_mut().unwrap()}.read(); // make the terminal read
		//let chr = buf as char;
		
		//let chr = ui.debug_pty_read();
		
		let chr = read_char(move||{unsafe{CURRENT_PTY.as_mut().unwrap()}.read()});
		
        buff.write_raw(chr);
		

        if to_update >= 1024 || (chr == '\0' && to_update != 0) { // if there are no new bytes comming show them, if there are more than 1024 bytes pending for being shown show them too
            to_update = 0;

            ui.update(buff.dom_mut());
        }

        else if chr != '\0' {
            to_update += 1; // count read bytes
        }

        else {
            thread::sleep(Duration::from_millis(2)); // save cpu time
        }

        WebView::handle_once();

    }


}
