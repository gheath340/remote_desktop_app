use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use pixels::{Pixels, SurfaceTexture};
use std::error::Error;

//initalize viewing window
pub fn window_init(payload: &[u8]) -> Result<(), Box<dyn Error>> {
    //create event loop and window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Remote desktop client")
        .build(&event_loop)?;

    //window dimensions
    let width = 800;
    let height = 600;

    //create pixel surface
    let surface_texture = SurfaceTexture::new(width, height, &window);
    let mut pixels = Pixels::new(width, height, surface_texture)?;

    //runs program, redraws when window requests, and closes when requested
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            //redraw screen
            Event::RedrawRequested(_) => {
                if let Err(e) = client_handle_frame_full(&payload, &mut pixels) {
                    eprintln!("Error handling frame: {e}");
                }
                //pushes to screen
                pixels.render().unwrap();
            }
            //handles closing of window
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => {}
            },

            _ => {}
        }
    });
}

//load frame and save as png in client dir
fn client_handle_frame_full(payload: &[u8], pixels: &mut pixels::Pixels) -> Result<(), Box<dyn Error>> {
    let img = image::load_from_memory(payload)?;
    let rgba = img.to_rgba8();

    let extent = pixels.texture().size();
    let frame = pixels.frame_mut();
    let win_width = extent.width;
    let win_height = extent.height;

    if rgba.width() as usize != win_width.try_into().unwrap() || rgba.height() as usize != win_height.try_into().unwrap() {
        println!("Frame size {} {} does not match window size {} {}", rgba.width(), rgba.height(), win_width, win_height);
        return Ok(());
    }

    frame.copy_from_slice(&rgba);

    Ok(())
}