use HUI::*;
use std::{thread, time::Duration};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::io::BufRead;
use std::cmp::min;

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


// supress output in release builds
macro_rules! eprintln {
    ($($rest:tt)*) => {
        #[cfg(debug_assertions)]
        std::eprintln!($($rest)*)
    }
}


struct OPTIONS {
    shell: String, // your shell or any other command (with or without arguments but no bash operators; if you want bash to create console pauser or pipes or whatever, just use sh -c)
	shell_args: Vec<String>,  // arguments for the shell (if loaded from the config file, args are part of the shell, so just parse them out)
    term: String, // terminal type to be advertised by termila to the shell (possible values: dumb, vt100, xterm, xterm-265color); linux-only
	max_buff_size: usize,
    /*
	ai_url: String, // url of OpenAI API server
    ai_key: String, // OpenAI API key
    ai_model: String, // OpenAI API model
    ai_prompt: String, // OpenAI API system prompt
	*/
    // TODO: color_override: String, // CSS function(s) to modify colors
    // TODO: bell_audio: String, // bell audio file
    saved_commands_file: String, // file with saved commands
	history_file: String, // file with shell history (to allow history modifications)
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
		
		// max_buff_size
		let max_buff_size = usize::MAX;
		
		
		// saved_commands_file, history_file
		
		let saved_commands_file = std::env::var("TERMILA_SAVED_COMMANDS").unwrap_or_default();
		
		let mut history_file: String;
		#[cfg(target_os = "linux")]
		{ history_file = std::env::var("HOME").unwrap_or_default()+"/."+&shell+"_history"; }
		#[cfg(target_os = "windows")]
		{ history_file = "".to_string(); } // windows cmd.exe doesnt store history


		return Self {shell, shell_args, term, max_buff_size, saved_commands_file, history_file, };
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
			<head></head>
            <body style="position: relative;">
			
				<!-- TERMINAL SPACE -->
                <p id="console" onclick="document.querySelectorAll('#menu button:has(+ div.popup)').forEach(f=>f.dataset.checked='false');/*document.querySelectorAll('#menu div').forEach(f=>f.style.visibility='hidden');*/" style="-webkit-user-select: text; margin: 0;  text-wrap: nowrap;"></p>
				<script>
				
					function term_type(what) {
						
						if (typeof what == 'object'){ // key event
						
							if (document.activeElement.tagName != 'BODY'){return;} // allow interaction with other inputs too
						
							// TODO: other non letter characters
							
							if (what.ctrlKey && what.keyCode >= 65 && what.keyCode <= 90) { // ctrl a..z
								if (what.ctrlKey && what.keyCode >= 67 && window.getSelection().toString() != '') {return;} // allow ctrl c copy
								term_type( what.keyCode-64 );
								what.preventDefault();
							}
							
							else if (what.keyCode == 38 && what.type == 'keydown') { // up
								term_type(27);
								term_type(91);
								term_type(65);
								what.preventDefault();
							}
							else if (what.keyCode == 40 && what.type == 'keydown') { // down
								term_type(27);
								term_type(91);
								term_type(66);
								what.preventDefault();
							}
							else if (what.keyCode == 39 && what.type == 'keydown') { // right
								term_type(27);
								term_type(91);
								term_type(67);
								what.preventDefault();
							}
							else if (what.keyCode == 37 && what.type == 'keydown') { // left
								term_type(27);
								term_type(91);
								term_type(68);
								what.preventDefault();
							}
							
							else if (event.keyCode == 27) { // esc
								term_type(27);
								what.preventDefault();
							}
							
							else if (event.keyCode == 9) { // tab
								term_type(9);
								what.preventDefault();
							}
						
							else if (event.keyCode == 8) { // backspace
								term_type(8);
								what.preventDefault();
							}
							
							else if (what.charCode != 0) { term_type(String.fromCharCode(what.charCode)); what.preventDefault(); }
							
						}
						
						else if (typeof what == 'string') { // text
							// encode string to bytes and then send it to terminal
							const encoder = new TextEncoder(); // default is utf-8
							const bytes = encoder.encode(what);
							bytes.forEach( b => term_type(b) );
						}
						
						else if (typeof what == 'number') { // byte
							// type directly
							key_term_handle(what);
						}
						
					}
					

					document.addEventListener('keydown', function(event) { term_type(event); });

					document.addEventListener('keypress', function(event) { term_type(event); });

					document.addEventListener('paste', function(event) {
						
						// allow interaction with other inputs too
						if (document.activeElement.tagName != 'BODY'){return;}

						// stop data actually being pasted
						event.stopPropagation();
						event.preventDefault();

						// get data
						var clipboardData = event.clipboardData || window.clipboardData;
						term_type(clipboardData);
						
					});
					
					document.addEventListener("selectionchange", (event) => {
						// dont allow interactions with other inputs delete our selection
						if (document.activeElement.tagName != 'BODY'){return;}
						
						// save selected text
						document.querySelector('#console').dataset.selection=window.getSelection().toString();

						// TODO: save and restore selection on focus out/in
					});


				</script>
				
				
				<!-- POPUP BUTTONS -->
                <div id="menu">
                	<style>
                    	
                        #menu {
                        	position: fixed;
                            top: 10px;
                            right: 10px; 
                            width: 30px;
                            max-height: 100vh;
                    	}
                        
                        #menu > button {
                        	width: 30px; 
                            height: 30px; 
                            min-width: unset;
                            margin: 5px 0;
                            padding: unset;
							
							opacity: 0;
							transition: opacity 1s;
                        }
						
						#menu:hover > button, #menu:has(div[style*="visibility: visible;"]) > button, #menu:has(button[data-checked='true'] + div.popup) > button {
							opacity: 1;
							transition: opacity 1s;
						}
                        
						
                        #menu div.popup {
                            position: absolute;
                            translate: 0px -30px;
                            right: 100%;
                            width: 300px; 
                            height: 300px; 
							margin-right: 30px;
							
							background-color: var(--hui_style_background_color);
							opacity: 0.8;
							padding: 5px;
							border-radius: 5px;
							overflow-y: scroll;
							overflow-x: hidden;
							
							visibility: hidden;
							transition: visibility 0.5s;
                        }
						
						#menu button[data-checked='true'] + div.popup {
							visibility: visible;
							transition: visibility 0.5s;
						}
						
						
                        #menu div.toast {
                            position: absolute;
                            translate: 0px -30px;
                            right:100%;
                            width: auto; 
                            max-height: 30px; 
							margin-right: 30px;
							
							background-color: var(--hui_style_background_color);
							opacity: 0.8;
							padding: 5px;
							border-radius: 5px;
							text-wrap: nowrap; 
							text-align: start;
							
							visibility: hidden;
							transition: visibility 0.5s;
                        }
						
						#menu button:hover + div.toast {
							visibility: visible;
							transition: visibility 0.5s;
						}
						
                    </style>
					
                </div>
				
            </body>
        </html>"#);
        //webview.html_element("body p", "", ""); // HUI bug


        // add keypress callback
        let key_term_handle = webview.call_native( move |args| {
                if let Some(arg) = args.get(0) {
                    if let Ok(val) = arg.parse::<u8>() {
                        //unsafe{CURRENT_PTY.as_ref().unwrap()}.write(val);
						unsafe{CURRENT_PTY.as_mut().unwrap()}.write(val);
                    }
                }
            }, None );
        webview.call_js(&format!("var key_term_handle = {};", key_term_handle), Some(false));
       
	   
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
				
				console.log(cols, rows);

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
		
		
		let self_ = Self { webview };
		
		
		// popups
		
		//webview.call_js(&format!("document.querySelector('#menu button#saved').onclick = function(){{ if (this.style.visibility=='hidden'){{ this.style.visibility=''; ( {} )(); }}else{{ this.style.visibility='hidden'; }} }}", webview.call_native( /*move*/ |args| {  /*webview.call_js(&format!("document.querySelector('#menu div#saved').innerHTML = {}", UI::popup_saved()),Some(false));*/  }, None)), Some(false));
		// TODO: HUI limitation - doesnt allow HUI calls within call_native 

		self_.popup_ai();
		self_.popup_saved();
		self_.popup_history();
		self_.popup_autoscroll();
		self_.popup_debug();

		self_
    }
	
	
	fn add_popup(&self, id: &str, button: &str, body: &str, is_toast_not_popup: bool){
		//self.webview.call_js( &format!( r#" document.querySelector('#menu').innerHTML += `<button id="{}" onclick="if(document.querySelector('#menu div#'+this.id).className=='toast'){{return;}} if (document.querySelector('#menu div#'+this.id).style.visibility=='visible'){{document.querySelector('#menu div#'+this.id).style.visibility='hidden';}} else{{document.querySelector('#menu div#'+this.id).style.visibility='visible';}}">{}</button> <div id="{}" class="{}">{}</div>`; "# , id, button, id, if is_toast_not_popup {"toast"} else {"popup"}, body) , Some(false));
		self.webview.call_js( &format!( r#" document.querySelector('#menu').innerHTML += `<button id="{}" onclick="this.dataset.checked = (!(this.dataset.checked=='true')).toString()">{}</button> <div id="{}" class="{}">{}</div>`; "# , id, button, id, if is_toast_not_popup {"toast"} else {"popup"}, body) , Some(false));
		// you can edit the element any time later using js
		// warning: js inside <script> or onload="" will not get executed 
	}
	

	fn popup_ai (&self) {
		self.add_popup(
			"ai",
			"AI", 
			r#"
			<h3>ASK AI</h3>
			<input type="text" onchange="
				const API_KEY = 'YOUR_API_KEY_HERE';
				(async function () {
					document.activeElement.blur();
					const response = await fetch('http://127.0.0.1:8080/v1/chat/completions' /*'https://api.openai.com/v1/chat/completions'*/, {
						method: 'POST',
						headers: {
							Authorization: 'Bearer '+API_KEY,
							'Content-Type': 'application/json',
						},
						body: JSON.stringify({
							model: 'gpt-4o-mini',
							messages: [
								{ role: 'system', content: 'You are a helpful assistant. \\nYou help the user with terminal interaction by explaining commands, giving solutions to errors and evaluating safety of commands. \\nAnswer shortly, dont use markdown.' },
								{ role: 'user', content: document.querySelector('#console').dataset.selection+'\\n\\n'+document.querySelector('div#ai input').value }
							],
							max_tokens: 50
						}),
				  });
				  const data = await response.json();
				  console.log(data);
				  //this.parentElement.lastChild.innerText = data.choices[0].message.content;
				  document.querySelector('#menu div#ai p').innerText = data.choices[0].message.content;
				})();
			">
			<h3>RESPONSE</h3>
			<p></p>
			"#, 
			false
		);
		// TODO: support continuous chat and chat export
	}
	
	fn popup_saved (&self) {
		let body = (||{
			let mut file = match File::open(unsafe{CURRENT_OPTIONS.as_mut().unwrap()}.saved_commands_file.clone()) {
				Ok(f) => f,
				Err(_) => return "<p>SAVED COMMANDS FILE NOT FOUND!</p>".to_string(),
			};
			let mut reader = BufReader::new(file);
			
			let mut html = "<p onclick=\"\" tabindex=\"0\" style=\"border-radius: 3px; border: 2px solid var(--hui_style_theme_color); padding: 3px;\">".to_string();
			for line0 in reader.lines() {
				let line = line0.unwrap(); 
				html.push_str(&line);
				html.push_str("<br>");
				if line == "" {
					html.push_str("</p><p onclick=\"term_type(this.innerText.split('\\\\n').filter(f => f != '').findLast(f=>true));\" tabindex=\"0\" style=\"border-radius: 3px; border: 2px solid var(--hui_style_theme_color); padding: 3px;\">");
				}
			}
			html.push_str("</p>");

			return html; 
		})();
		self.add_popup("saved", "SS", &body, false);
	}
		
	fn popup_history (&self) {
		let body = (||{
			let mut file = match File::open(unsafe{CURRENT_OPTIONS.as_mut().unwrap()}.history_file.clone()) { // TODO: auto-reload on each open
				Ok(f) => f,
				Err(_) => return "<p>HISTORY FILE NOT FOUND!</p>".to_string(),
			};
			let start = -(file.metadata().expect("REASON").len().saturating_sub(10240) as i64);
			let mut reader = BufReader::new(file);
			reader.seek(SeekFrom::End(start)); // TODO: auto-load all previous (efficiently)
			return reader.lines().map(|line| format!("<p onclick=\"term_type(this.innerHTML);\" tabindex=\"0\" style=\"border-radius: 3px; border: 2px solid var(--hui_style_theme_color); padding: 3px;\">{}</p>", line.unwrap())).collect(); 
		})();
		self.add_popup("history", "HI", &body, false);
	}

	fn popup_autoscroll (&self) {
		self.add_popup(
			"autoscroll",
			"AS", 
			r#"
			<span></span>
			<style>
				button#autoscroll[data-checked='true'] + div span::before {
					content: 'auto-scroll disabled';
				}
				#autoscroll:not(button#autoscroll[data-checked='true']) + div span::before {
					content: 'auto-scroll enabled';
				}
			</style>
			"#, 
			true
		);
	}

	fn popup_debug (&self) {
		self.add_popup(
			"dbg",
			"DG", 
			r#"
			<p>DEBUG</p>
			<br>
			<input type="text" id="buffer">
			<br>
			<button id="submit" onclick="this.checked = true;">submit</button>
			<br>
			<button id="read" onclick="this.checked = true;">read</button>
			"#, 
			false
		);
	}

	// TODO: custom popup_* -> plugin interface = just shared object with one function `void termila_custom_popup_init(void* webview, function add_popup);`
	
	
	fn escape_text (text: &String) -> String {
		let mut result = String::with_capacity(text.len());
		for c in text.chars() {
			match c {
				' '  => result.push_str("&nbsp;"),
				'\\' => result.push_str("\\\\"),
				'<'  => result.push_str("&lt;"),
				'>'  => result.push_str("&gt;"),
				'\n' => result.push_str("<br>"),
				'`'  => result.push_str("\\`"),
				_    => result.push(c),
			}
		}
		return result;
	}

	
	fn debug_pty_read(&mut self) -> char {
		
		self.webview.call_js("document.querySelector('div#dbg').style.display='';", Some(false));
		
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
    text: String, // console text
    style: HashMap<&'l str, &'l str>, // css attributes
    updated: bool, // changet but not displayed
    id: usize, // html id, 0 means unset, set when update runs, '#t-<value>'
}
struct BUFF<'a> {
    formated_text: Vec<BUFF_formated_text<'a>>, // html_vec [ [<html>,<css or classn>,<q updated>], ] // TODO: ensure that blanks are not preserved
    formated_text_last_id: usize,
    formated_text_changes: usize,
	
    current_escape: String, // multi-character special commands; contains the sequence from the escape byte to the last character read; if we are not currently reading any sequence (after previous was finished) it is empty string
    current_escape_max_length: usize, // this is to avoid breaking terminal with unsupported/malicious sequences; the value depends on sequence type
    
	cursor_position_index: usize,
    cursor_position_character: usize,
	
    handle_cr_next_time: bool, // TODO: remove
}
impl BUFF<'_> {
	
    fn new() -> Option<Self> {
        unsafe {
            Some(Self {
                formated_text: vec![/*BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:false,id:0},*/],
                formated_text_last_id: 0,
				formated_text_changes: 0,
				current_escape: "".to_string(),
                current_escape_max_length: 0,
                cursor_position_index: 0,
                cursor_position_character: 0,
                handle_cr_next_time: false,
            })
        }
    }

    
	fn write_buff(&mut self, chr: char) {

		// fix invalid cursor position
        if self.cursor_position_index >= self.formated_text.len(){
            eprintln!("BUFF: (warning) invalid position in buffer - reseting");
            self.cursor_position_index = self.formated_text.len();
            self.cursor_position_character = 0;
            self.formated_text.push( BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:true,id:0} );
			self.formated_text_changes += 1;
        }
        if self.cursor_position_character > self.formated_text.get(self.cursor_position_index).unwrap().text.len() {
            eprintln!("BUFF: (warning) invalid position in buffer - reseting");
            self.cursor_position_character = self.formated_text.get(self.cursor_position_index).unwrap().text.len();
        }
		
		
        // remove character at cursor position if overwriting and if its not newline
        let mut index = self.cursor_position_index;
        let mut character = self.cursor_position_character;
		if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") == "\0" { // move to at character position if we are not already
			self.iter_next(& mut index,& mut character);
		}
		
        if self.formated_text.get(index).unwrap().text.get(character..character+1).unwrap_or("\0") != "\n" {

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
			
			if !self.formated_text.get(index).unwrap().updated {
				self.formated_text.get_mut(index).unwrap().updated = true;
				self.formated_text_changes += 1;
			}

        }
		

        // place character
        let mut pos = self.cursor_position_character;
        while ! self.formated_text.get(self.cursor_position_index).unwrap().text.is_char_boundary(pos) && pos > 0 {
            pos-=1;
        }
        self.formated_text.get_mut(self.cursor_position_index).unwrap().text.insert(pos, chr);
		if !self.formated_text.get(self.cursor_position_index).unwrap().updated {
			self.formated_text.get_mut(self.cursor_position_index).unwrap().updated = true;
			self.formated_text_changes += 1;
		}
        self.cursor_position_character += chr.len_utf8();
		
    }

    fn write_raw(&mut self, mut chr: char) {

		if chr == '\x00' {return;} // never accept '\0' for processing - pty implementation returns it when there are no new bytes (it isnt shown anyway and even escape sequences wont contain it)


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
				
				// ending sequence
				if (0x40..=0x7E).contains(self.current_escape.as_bytes().last().unwrap()) && self.current_escape.len() > 2 {
					
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
							if !self.formated_text.get(self.cursor_position_index).unwrap().updated {
								self.formated_text.get_mut(self.cursor_position_index).unwrap().updated = true;
								self.formated_text_changes += 1;
							}
						}
						// create new style field and switch to it
						else {
							if self.cursor_position_index < self.formated_text.len() { // expected
								self.formated_text.insert(self.cursor_position_index+1, BUFF_formated_text{text:"".to_string(),style:self.formated_text.get(self.cursor_position_index).unwrap().style.clone(),updated:true,id:0});
								self.cursor_position_index += 1;
								self.cursor_position_character = 0;
							}
							else { // handle situation when cursor position is wrong = len() is 0 (this is result of partial updates removing all segments - even the first one)
								self.formated_text.push(BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:true,id:0});
								self.cursor_position_index = self.formated_text.len()-1;
								self.cursor_position_character = 0;
								eprintln!("BUFF: (warning) invalid position in buffer - reseting");
							}

							self.formated_text.get_mut(self.cursor_position_index).unwrap().style.extend(css);
							self.formated_text_changes += 1;
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
						
						if final_escape.ends_with("2J") || final_escape.ends_with("3J") { // entire screen
							// TODO: there are actually some differences between these two
							for i in (0..self.formated_text.len()) {
								self.formated_text.get_mut(i).unwrap().text = "".to_string();
								if !self.formated_text.get_mut(i).unwrap().updated {
									self.formated_text.get_mut(i).unwrap().updated = true;
									self.formated_text_changes += 1;
								}
							}
							self.cursor_position_index = 0;
							self.cursor_position_character = 0;
						}
						else if final_escape.ends_with("1J") { // from beginning to cursor
							for i in (0..self.cursor_position_index) {
								self.formated_text.get_mut(i).unwrap().text = "".to_string();
								if !self.formated_text.get_mut(i).unwrap().updated {
									self.formated_text.get_mut(i).unwrap().updated = true;
									self.formated_text_changes += 1;
								}
							}
							self.formated_text.get_mut(self.cursor_position_index).unwrap().text.drain(..self.cursor_position_character);
							if !self.formated_text.get_mut(self.cursor_position_index).unwrap().updated {
								self.formated_text.get_mut(self.cursor_position_index).unwrap().updated = true;
								self.formated_text_changes += 1;
							}
							self.cursor_position_character = 0;
						}
						else /*final_escape.ends_with("0J") || final_escape.ends_with("J")*/ { // from end to cursor 
							for i in (self.cursor_position_index+1..self.formated_text.len()) {
								self.formated_text.get_mut(i).unwrap().text = "".to_string();
								if !self.formated_text.get_mut(i).unwrap().updated {
									self.formated_text.get_mut(i).unwrap().updated = true;
									self.formated_text_changes += 1;
								}
							}
							self.formated_text.get_mut(self.cursor_position_index).unwrap().text.truncate(self.cursor_position_character+1);
							if !self.formated_text.get_mut(self.cursor_position_index).unwrap().updated {
								self.formated_text.get_mut(self.cursor_position_index).unwrap().updated = true;
								self.formated_text_changes += 1;
							}
						}

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
        self.formated_text.insert(index+1, BUFF_formated_text{text:self.formated_text.get(index).unwrap().text.get(..character).unwrap_or("<TERMILA_PARSER_ERROR>").to_string(),style:self.formated_text.get(index).unwrap().style.clone(),updated:true,id:0});
        // insert new patr to index+2
        self.formated_text.insert(index+2, BUFF_formated_text{text:"".to_string(),style:self.formated_text.get(self.cursor_position_index).unwrap().style.clone(),updated:true,id:0});
        // insert second part to index+3
        self.formated_text.insert(index+3, BUFF_formated_text{text:self.formated_text.get(index).unwrap().text.get(character..).unwrap_or("<TERMILA_PARSER_ERROR>").to_string(),style:self.formated_text.get(index).unwrap().style.clone(),updated:true,id:0});
        // remove part at index
		self.formated_text.get_mut(index).unwrap().text = "".to_string();
		if !self.formated_text.get_mut(index).unwrap().updated {
			self.formated_text.get_mut(index).unwrap().updated = true;
			self.formated_text_changes += 1;
		}

        self.cursor_position_index = index+2;
        self.cursor_position_character = 0;
		
		self.formated_text_changes += 3;

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
		self.formated_text.insert(self.cursor_position_index+1, BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:true,id:0}); // use neutral color
		self.formated_text_changes += 1;
		self.cursor_position_index += 1;
		self.cursor_position_character = 0;
		for _ in 0..column { self.write_buff(' '); } // add spaces without style
		self.formated_text.insert(self.cursor_position_index+1, BUFF_formated_text{text:"".to_string(),style:self.formated_text.get(self.cursor_position_index-1).unwrap().style.clone(),updated:true,id:0}); // restore previous color
		self.formated_text_changes += 1;
		self.cursor_position_index += 1;
		self.cursor_position_character = 0;
		


		// print debug statistics
        eprintln!("SET CURSOR POSITION (c{},r{}) -> (c{},r{}) -> (c{},r{}) = old->desired->final", old_col,old_row, des_col,des_row, self.get_cursor_c(),self.get_cursor_r() );

    }


    fn update_full (&mut self, webview: &HUI::WebView) { // full terminal update (slow)
	
		if self.formated_text_changes == 0 { return; }  // nothing to update

        // create html
        let mut html = String::new();
        for i in &self.formated_text {
            html.push_str( &format!("<span style=\"{}\">{}</span>", i.style.iter().map(|(key, value)|format!("{}: {};", key, value)).collect::<Vec<String>>().join(" "), UI::escape_text(&i.text)) );
        }

        // update whole terminal content
        let js_command = format!("document.querySelector('body p').innerHTML=`{}`;", html.replace("`","\\`"));
        webview.call_js(&js_command, Some(false));

        // TODO: clear blanks here to sync with html dom, apply clear sequences (segments should have an id paired with dom id, if it gets updated to blank both get deleted here) -- part of partial updates, which it will be possible to implement as soon as the writing/positioning is reliable enough

        // autoscroll
        webview.call_js("if (document.querySelector('#menu button#autoscroll').dataset.checked!='true') {window.scrollTo(0, document.body.scrollHeight);}", Some(false));
		// TODO: better scrolling (hide initial free lines + scroll to current cursor position)
		
		self.formated_text_changes = 0; // everything is updated now 

    }

    fn update_partial (&mut self, webview: &HUI::WebView) { // partial terminal update (little faster)
	
		if self.formated_text_changes == 0 { return; }  // nothing to update
		
		// clear old scrollback to save memory
		
		// TODO: issue - know size without iterating whole terminal or guess it
		//unsafe{CURRENT_OPTIONS.as_mut().unwrap()}.max_buff_size
		
		
		// perform update
		for i in (0..self.formated_text.len()).rev() {
			
			if self.formated_text_changes == 0 { break; }  // everything was already updated
			
			if self.formated_text.get(i).unwrap().updated {
				
				if self.formated_text.get(i).unwrap().id == 0 { // add element
					self.formated_text_last_id += 1;
					self.formated_text.get_mut(i).unwrap().id = self.formated_text_last_id;
					
					webview.call_js( 
						&format!(
							"(function(){{ let e = document.createElement('span'); e.id = 't-{}'; e.innerHTML=`{}`; e.style.cssText = `{}`; {} }})()",
							self.formated_text.get(i).unwrap().id,
							UI::escape_text(&self.formated_text.get(i).unwrap().text),
							self.formated_text.get(i).unwrap().style.iter().map(|(key, value)|format!("{}: {};", key, value)).collect::<Vec<String>>().join(" "),
							if self.formated_text.get(i+1).unwrap_or(&BUFF_formated_text{text:"".to_string(),style:[].iter().cloned().collect(),updated:false,id:0}).id != 0 {format!("document.querySelector('body p#console span#t-{}').before(e);", self.formated_text.get(i+1).unwrap().id)} else {"document.querySelector('body p#console').appendChild(e);".to_string()}
						), 
						Some(false) 
					);
					
					self.formated_text.get_mut(i).unwrap().updated = false;
				}
				
				else if self.formated_text.get(i).unwrap().text == "" { // delete element
					webview.call_js( 
						&format!(
							"(function(){{ document.querySelector('body p#console span#t-{}').remove(); }})()",
							self.formated_text.get(i).unwrap().id,
						), 
						Some(false) 
					);
					if self.cursor_position_index == i { 
						if i != 0 {
							self.cursor_position_index -= 1;
							self.cursor_position_character = self.formated_text.get(self.cursor_position_index).unwrap().text.len();
						}
						else {
							self.cursor_position_index = 0;
							self.cursor_position_character = 0;
						}
						 
					}
					else if self.cursor_position_index > i {
						self.cursor_position_index -= 1;
					}
					self.formated_text.remove(i); // should be safe to do since we iterate from the end
				}
				
				else { // edit element
					webview.call_js( 
						&format!(
							"(function(){{ let e = document.querySelector('body p#console span#t-{}'); e.innerHTML=`{}`; e.style.cssText = `{}`; }})()",
							self.formated_text.get(i).unwrap().id,
							UI::escape_text(&self.formated_text.get(i).unwrap().text),
							self.formated_text.get(i).unwrap().style.iter().map(|(key, value)|format!("{}: {};", key, value)).collect::<Vec<String>>().join(" ")
						), 
						Some(false) 
					);
					
					self.formated_text.get_mut(i).unwrap().updated = false;
				}
				
				self.formated_text_changes-=1;
			}
			
		}
		
		if self.formated_text_changes > 0 { 
			eprintln!("BUFF: (warning) changes updated don't match changes made"); 
			#[cfg(not(debug_assertions))] 	
			{ self.formated_text_changes = 0; }
		}
		
		
        // autoscroll
        webview.call_js("if (document.querySelector('#menu button#autoscroll').dataset.checked!='true') {window.scrollTo(0, document.body.scrollHeight);}", Some(false));
		// TODO: better scrolling (hide initial free lines + scroll to current cursor position)
		
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
	
	write_cache: Vec<u8>
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
			
            Some( Self{ in_write, out_read, hpc, pi, rows: 999, columns: 999, write_cache: vec![], })
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
		if b == 0x1B || b == b'[' {
			self.write_cache.push(b);
			return true;
		}
		return
		if self.write_cache.is_empty(){
			unsafe {
				//let mut written = 0u32;
				!WriteFile(
					self.in_write,
					Some(&[b]), // data to be written  // TODO: windows doesnt allow us to write vt sequences by byte so current code doesnt work but this does: Some(&[27,91,65]), current solution is workaround
					None, //Some(&mut written as *mut u32), // number of bytes written (optional, not needed)
					None,
				)
				.is_err()
			}
		}
		else {
			self.write_cache.push(b);
			unsafe {
				!WriteFile(
					self.in_write,
					Some(&self.write_cache),
					None,
					None,
				)
				.is_err();
			}
			self.write_cache = vec![];
			true
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
static mut CURRENT_OPTIONS: Option<OPTIONS> = None;

fn main() {
	
	// load options
	let options = OPTIONS::new();
	unsafe {CURRENT_OPTIONS = Some(OPTIONS::new());}

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

        //let buf = unsafe{CURRENT_PTY.as_mut().unwrap()}.read(); // make the terminal read
		//let chr = buf as char;
		
		//let chr = ui.debug_pty_read();
		
		let chr = read_char(move||{unsafe{CURRENT_PTY.as_mut().unwrap()}.read()});
		
        buff.write_raw(chr);
		

        if to_update >= 1024 || (chr == '\0' && to_update != 0) { // if there are no new bytes comming show them, if there are more than 1024 bytes pending for being shown show them too
            to_update = 0;

            //buff.update_full(&mut ui.webview);
			buff.update_partial(&mut ui.webview);
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
