use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AudioContext, GainNode, MediaDeviceInfo, MediaDeviceKind, OscillatorNode, OscillatorType};
use std::cell::RefCell;
use std::rc::Rc;

struct PlaybackState {
    ctx: Option<AudioContext>,
    osc: Option<OscillatorNode>,
    timer_handle: Option<i32>,
}

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    let state = Rc::new(RefCell::new(PlaybackState {
        ctx: None,
        osc: None,
        timer_handle: None,
    }));

    // Populate initial devices
    refresh_device_list().await?;

    setup_permission_button();
    setup_playback_toggle(state);

    Ok(())
}

async fn refresh_device_list() -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let doc = window.document().unwrap();
    let select: web_sys::HtmlSelectElement = doc.get_element_by_id("device-select").unwrap().dyn_into()?;

    let nav = window.navigator();
    if let Ok(media_devices) = nav.media_devices() {
        if let Ok(promise) = media_devices.enumerate_devices() {
            let devices_val = wasm_bindgen_futures::JsFuture::from(promise).await?;
            let devices: js_sys::Array = devices_val.into();

            select.set_inner_html(""); // Clear old options

            let mut count = 0;
            for i in 0..devices.length() {
                let info: MediaDeviceInfo = devices.get(i).dyn_into()?;
                if info.kind() == MediaDeviceKind::Audiooutput {
                    let opt: web_sys::HtmlOptionElement = doc.create_element("option")?.dyn_into()?;
                    let label = info.label();
                    let label_str = if label.is_empty() {
                        format!("Audio Output {}", count + 1)
                    } else {
                        label
                    };
                    opt.set_text(&label_str);
                    opt.set_value(&info.device_id());
                    select.append_child(&opt)?;
                    count += 1;
                }
            }

            if count == 0 {
                let opt: web_sys::HtmlOptionElement = doc.create_element("option")?.dyn_into()?;
                opt.set_text("Default Output Device");
                opt.set_value("default");
                select.append_child(&opt)?;
            }
        }
    }
    Ok(())
}

fn setup_permission_button() {
    let doc = web_sys::window().unwrap().document().unwrap();
    let perm_btn = doc.get_element_by_id("perm-btn").unwrap();

    let cb = Closure::wrap(Box::new(move || {
        wasm_bindgen_futures::spawn_local(async {
            let window = web_sys::window().unwrap();
            // Browsers only expose output device labels if user granted microphone permission at least once
            if let Ok(media_devices) = window.navigator().media_devices() {
                let constraints = web_sys::MediaStreamConstraints::new();
                constraints.set_audio(&JsValue::from_bool(true));
                if let Ok(promise) = media_devices.get_user_media_with_constraints(&constraints) {
                    if let Ok(stream_val) = wasm_bindgen_futures::JsFuture::from(promise).await {
                        let stream: web_sys::MediaStream = stream_val.dyn_into().unwrap();
                        for track in stream.get_tracks().to_vec() {
                            let track: web_sys::MediaStreamTrack = track.dyn_into().unwrap();
                            track.stop(); // Immediately release microphone
                        }
                    }
                }
            }
            let _ = refresh_device_list().await;
        });
    }) as Box<dyn FnMut()>);

    perm_btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).unwrap();
    cb.forget();
}

fn setup_playback_toggle(state: Rc<RefCell<PlaybackState>>) {
    let doc = web_sys::window().unwrap().document().unwrap();
    let play_btn: web_sys::HtmlButtonElement = doc.get_element_by_id("play-btn").unwrap().dyn_into().unwrap();

    let cb = Closure::wrap(Box::new(move || {
        let is_playing = state.borrow().osc.is_some();

        if is_playing {
            stop_playback(&mut state.borrow_mut());
        } else {
            let doc = web_sys::window().unwrap().document().unwrap();
            let input: web_sys::HtmlInputElement = doc.get_element_by_id("duration-input").unwrap().dyn_into().unwrap();
            let select: web_sys::HtmlSelectElement = doc.get_element_by_id("device-select").unwrap().dyn_into().unwrap();

            let minutes: f64 = input.value().parse().unwrap_or(1.0);
            let device_id = select.value();

            start_playback(minutes, device_id, state.clone());
        }
    }) as Box<dyn FnMut()>);

    play_btn.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).unwrap();
    cb.forget();
}

fn start_playback(minutes: f64, device_id: String, state: Rc<RefCell<PlaybackState>>) {
    let window = web_sys::window().unwrap();
    let doc = window.document().unwrap();

    let ctx = match AudioContext::new() {
        Ok(c) => c,
        Err(_) => return,
    };

    // Route audio via setSinkId if supported by the browser (Chromium/Safari)
    if js_sys::Reflect::has(&ctx, &JsValue::from_str("setSinkId")).unwrap_or(false) {
        let func_val = js_sys::Reflect::get(&ctx, &JsValue::from_str("setSinkId")).unwrap();
        if let Ok(func) = func_val.dyn_into::<js_sys::Function>() {
            let _ = func.call1(&ctx, &JsValue::from_str(&device_id));
        }
    }

    let osc = ctx.create_oscillator().unwrap();
    let gain = ctx.create_gain().unwrap();

    osc.set_type(OscillatorType::Sine);
    osc.frequency().set_value(440.0);

    // Prevent pop/click on start: ramp gain from 0.0 to 0.2 in 20ms
    let now = ctx.current_time();
    gain.gain().set_value_at_time(0.0, now).unwrap();
    gain.gain().linear_ramp_to_value_at_time(0.20, now + 0.02).unwrap();

    osc.connect_with_audio_node(&gain).unwrap();
    gain.connect_with_audio_node(&ctx.destination()).unwrap();

    let total_seconds = (minutes * 60.0).round() as i32;
    let end_time = now + total_seconds as f64;
    osc.start().unwrap();

    // Update UI elements
    let play_btn: web_sys::HtmlButtonElement = doc.get_element_by_id("play-btn").unwrap().dyn_into().unwrap();
    play_btn.set_text_content(Some("⏹ Stop Tone"));
    play_btn.set_class_name("playing");

    // Start UI Countdown timer
    let remaining_counter = Rc::new(RefCell::new(total_seconds));
    let state_for_timer = state.clone();

    let timer_closure = Closure::wrap(Box::new(move || {
        let doc = web_sys::window().unwrap().document().unwrap();
        let status = doc.get_element_by_id("status").unwrap();

        let mut rem = remaining_counter.borrow_mut();
        *rem -= 1;

        if *rem <= 0 {
            stop_playback(&mut state_for_timer.borrow_mut());
        } else {
            let mins = *rem / 60;
            let secs = *rem % 60;
            status.set_text_content(Some(&format!("Playing: {:02}:{:02}", mins, secs)));
        }
    }) as Box<dyn FnMut()>);

    let handle = window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            timer_closure.as_ref().unchecked_ref(),
            1000,
        )
        .unwrap();

    timer_closure.forget();

    state.borrow_mut().ctx = Some(ctx);
    state.borrow_mut().osc = Some(osc);
    state.borrow_mut().timer_handle = Some(handle);

    let status = doc.get_element_by_id("status").unwrap();
    status.set_text_content(Some(&format!(
        "Playing: {:02}:{:02}",
        total_seconds / 60,
        total_seconds % 60
    )));
}

fn stop_playback(state: &mut PlaybackState) {
    let window = web_sys::window().unwrap();
    let doc = window.document().unwrap();

    if let Some(handle) = state.timer_handle.take() {
        window.clear_interval_with_handle(handle);
    }

    if let Some(osc) = state.osc.take() {
        let _ = osc.stop();
    }

    if let Some(ctx) = state.ctx.take() {
        let _ = ctx.close();
    }

    let play_btn: web_sys::HtmlButtonElement = doc.get_element_by_id("play-btn").unwrap().dyn_into().unwrap();
    play_btn.set_text_content(Some("▶ Play Tone"));
    play_btn.set_class_name("");

    let status = doc.get_element_by_id("status").unwrap();
    status.set_text_content(Some("Stopped"));
}
