/*
	ATTENTION:
	you need to have a font.ttf in the current working directory of your VST host
	or change the path to the font below
	otherwise this plugin won't load correctly
*/

#[macro_use] extern crate log;
extern crate log_panics;
extern crate simplelog;
extern crate num_traits;
extern crate asprim;
#[macro_use] extern crate enum_primitive;
#[macro_use] extern crate vst2;
#[macro_use] extern crate easyvst;
extern crate winit;
#[macro_use] extern crate conrod;

use simplelog::*;

use num_traits::{FromPrimitive, Float};
use asprim::AsPrim;

use vst2::buffer::AudioBuffer;
use vst2::plugin::{Category, Info};
use vst2::host::Host;
use vst2::editor::Editor;

use easyvst::*;
use easyvst::util::*;

easyvst!(ParamId, MyState, MyPlugin);

enum_from_primitive! {
	#[repr(usize)]
	#[derive(Debug, Copy, Clone)]
	pub enum ParamId {
		GainDb,
	}
}

#[derive(Default)]
struct MyState {
	gain_amp: f32,
}

impl UserState<ParamId> for MyState {
	fn param_changed(&mut self, param_id: ParamId, val: f32) {
		info!("param_changed {:?} {:.3}", param_id, val.as_f32());
		use ParamId::*;
		match param_id {
			GainDb => self.gain_amp = db_to_amp(val),
		}
	}
}

type MyPluginState = PluginState<ParamId, MyState>;

#[derive(Default)]
struct MyPlugin {
	state: MyPluginState,
	ui: Option<UiState>,
}

impl MyPlugin {
	fn process_one_channel<T: Float + AsPrim>(&mut self, input: &[T], output: &mut [T]) {
		for (input_sample, output_sample) in input.iter().zip(output) {
			*output_sample = *input_sample * self.state.user_state.gain_amp.as_();
		}
	}
}

impl EasyVst<ParamId, MyState> for MyPlugin {
	fn state(&self) -> &MyPluginState {
		&self.state
	}

	fn state_mut(&mut self) -> &mut MyPluginState {
		&mut self.state
	}

	fn get_info(&self) -> Info {
		Info {
			name: "conrodgain".to_string(),
			vendor: "easyvst".to_string(),
			unique_id: 0x4567DCBA,
			category: Category::Effect,

			inputs: 2,
			outputs: 2,
			parameters: 1,

			..Info::default()
		}
	}

	fn params() -> Vec<ParamDef> {
		vec![
			ParamDef::new("Gain", (-48.0).as_(), 12.0.as_(), 0.0.as_()),
		]
	}

	fn format_param(param_id: ParamId, val: f32) -> String {
		info!("format_param {:?} {:.3}", param_id, val.as_f32());
		use ParamId::*;
		match param_id {
			GainDb => format!("{:.3} dB", val.as_f32()),
		}
	}

	fn new(state: MyPluginState) -> Self {
		let mut p: MyPlugin = Default::default();
		p.state = state;
		p
	}

	fn init(&mut self) {
		use std::fs::File;
		let _ = CombinedLogger::init(
			vec![
				WriteLogger::new(LogLevelFilter::Info, Config::default(), File::create("vst.log").unwrap()),
			]
		);
		info!("init in host {:?}", self.state.host.get_info());
	}

	fn process_f<T: Float + AsPrim>(&mut self, buffer: AudioBuffer<T>) {
		// split out the input and output buffers into two vectors
		let (inputs, outputs) = buffer.split();

		// for each buffer, transform the samples
		for (input_buffer, output_buffer) in inputs.iter().zip(outputs) {
			self.process_one_channel(input_buffer, output_buffer);
		}
	}

	fn get_editor(&mut self) -> Option<&mut Editor> { Some(self) }
}

use conrod::glium;
use conrod::backend::glium::glium::glutin::WindowBuilder;
use conrod::backend::glium::glium::DisplayBuild;

use std::os::raw::c_void;

pub struct UiState {
	pub display: glium::Display,
	pub ui: conrod::Ui,
	pub image_map: conrod::image::Map<glium::texture::Texture2d>,
	pub ids: Ids,
	pub renderer: conrod::backend::glium::Renderer,
}

#[derive(Debug)]
pub enum AppError {
	GetWindowFail,
	GetInnerSizeFail,
	LoadRendererFail,
}

impl UiState {
	pub fn new(display: glium::Display) -> Result<Self, AppError> {
		let (width, height) = display.get_window()
			.ok_or(AppError::GetWindowFail)
			.and_then({|window|
				window.get_inner_size().ok_or(AppError::GetInnerSizeFail)
			})?;

		info!("size : {}x{}", width, height);

		info!("framebuffer: {:?}", display.get_framebuffer_dimensions());

		let mut ui = conrod::UiBuilder::new([width as f64, height as f64]).build();

		ui.fonts.insert_from_file("font.ttf").unwrap();

		let renderer = match conrod::backend::glium::Renderer::new(&display) {
			Ok(r) => r,
			Err(e) => {
				error!("Error creating Renderer: {:?}", e);
				return Err(AppError::LoadRendererFail)
			},
		};

		let image_map = conrod::image::Map::new();
		let ids = Ids::new(ui.widget_id_generator());

		Ok(UiState { display: display, ui: ui, image_map: image_map, renderer: renderer, ids: ids })
	}

	fn draw(&mut self, state: &mut MyPluginState) {
		for event in self.display.poll_events() {
			// Use the `winit` backend feature to convert the winit event to a conrod one.
			if let Some(event) = conrod::backend::winit::convert(event.clone(), &self.display) {
				self.ui.handle_event(event);
			}
		}

		set_widgets(state, self.ui.set_widgets(), &mut self.ids);

		let mut target = self.display.draw();

		// Render the `Ui` and then display it on the screen.
		if let Some(primitives) = self.ui.draw_if_changed() {
			self.renderer.fill(&self.display, primitives, &self.image_map);
			self.renderer.draw(&self.display, &mut target, &self.image_map).unwrap();
		}

		target.finish().unwrap();
	}
}

fn set_widgets(state: &mut MyPluginState, ref mut ui: conrod::UiCell, ids: &mut Ids) {
	use conrod::{color, widget, Colorable, Labelable, Positionable, Sizeable, Widget, Borderable};

	widget::Canvas::new()
		.color(color::CHARCOAL)
		.border(0.0)
		.set(ids.body, ui);
	let gain_db = state.get_param(ParamId::GainDb);
	let (min, max) = {
		let def = state.get_param_def(ParamId::GainDb);
		(def.min, def.max)
	};
	let label = format!("Gain: {:.2} dB", gain_db);
	if let Some(val) = widget::Slider::new(gain_db, min, max)
		.w_h(300.0, 50.0)
		.middle_of(ids.body)
		.rgb(0.5, 0.3, 0.6)
		.border(1.0)
		.label(&label)
		.label_color(color::WHITE)
	.set(ids.gain_slider, ui) {
		state.set_param(ParamId::GainDb, val);
	}
	for _click in widget::Button::new()
		.middle_of(ids.body)
		.down_from(ids.gain_slider, 45.0)
		.w_h(200.0, 30.0)
		.color(color::RED)
		.label("click me")
	.set(ids.button, ui) {
		info!("Bing!");
	}
}

widget_ids! {
	pub struct Ids {
		body,
		button,
		gain_slider,
	}
}

const WINDOW_WIDTH: u32 = 640;
const WINDOW_HEIGHT: u32 = 480;

impl Editor for MyPlugin {
	fn size(&self) -> (i32, i32) {
		trace!("size");
		(WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32)
	}

	fn position(&self) -> (i32, i32) {
		trace!("position");
		(0, 0)
	}

	fn open(&mut self, parent: *mut c_void) {
		trace!("open {}", parent as usize);
		let wb = winit::WindowBuilder::new()
			.with_dimensions(WINDOW_WIDTH, WINDOW_HEIGHT)
			.with_decorations(false)
			.with_parent(parent);
		match WindowBuilder::from_winit_builder(wb)
			.with_multisampling(4)
			// .with_depth_buffer(24)
		.build_glium() {
			Ok(display) => {
				trace!("window created");
				match UiState::new(display) {
					Ok(ui) => self.ui = Some(ui),
					Err(e) => error!("creating ui failed: {:?}", e)
				}
				trace!("self.ui created");
			}
			Err(e) => error!("creating window failed: {:?}", e)
		}
	}

	fn close(&mut self) {
		trace!("close");
		self.ui = None;
	}

	fn idle(&mut self) {
		if let Some(ref mut ui) = self.ui {
			ui.draw(&mut self.state);
		}
	}

	fn is_open(&mut self) -> bool {
		trace!("is_open");
		self.ui.is_some()
	}
}