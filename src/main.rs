//! This is the source code of the "Windowing" chapter at http://vulkano.rs.
//!
//! It is not commented, as the explanations can be found in the book itself.

mod renderer;

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use renderer::Renderer;

fn main() {
	let event_loop = EventLoop::new().unwrap();
	event_loop.set_control_flow(ControlFlow::Wait);

	let mut app = App {renderer: None, window: None};

	event_loop.run_app(&mut app).unwrap();
}

struct App {
	renderer: Option<Renderer>,
	window: Option<Arc<Window>>,
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window_attributes = Window::default_attributes();
		self.window = Some(Arc::new(event_loop.create_window(window_attributes).unwrap()));
		self.renderer = Some(Renderer::new(event_loop, self.window.as_ref().unwrap()));
	}

	fn window_event(
		&mut self,
		event_loop: &ActiveEventLoop,
		_window_id: WindowId,
		event: WindowEvent,
	) {
		match event {
			WindowEvent::CloseRequested => {
				event_loop.exit();
			}
			WindowEvent::RedrawRequested => {
				if let Some(renderer) = &mut self.renderer {
					renderer.draw();
				}
				self.window.as_ref().unwrap().request_redraw();
			}
			_ => {}
		}
	}
}