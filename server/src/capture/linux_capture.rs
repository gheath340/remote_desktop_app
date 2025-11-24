use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};
use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SourceType, PersistMode,}
};
use ashpd::WindowIdentifier;
use pipewire as pw;
use pipewire::{
    main_loop::MainLoop,
    context::Context,
    stream::{Stream, StreamRef, StreamFlags},
    properties::properties,
    spa::utils::Direction,
    spa::pod::Pod,
};
use std::os::fd::{OwnedFd, FromRawFd};

pub fn start_sck_stream() -> Receiver<(usize, usize, Vec<u8>)> {
    let (tx, rx) = mpsc::channel::<(usize, usize, Vec<u8>)>();

    // Spawn a dedicated thread that runs async stuff (ashpd + pipewire)
    thread::spawn(move || {
        // Separate runtime for capture
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        if let Err(e) = rt.block_on(run_capture(tx)) {
            eprintln!("[linux_capture] error: {e}");
        }
    });

    rx
}

type CaptureResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn run_capture(tx: Sender<(usize, usize, Vec<u8>)>) -> CaptureResult<()> {
    // 1) Create Screencast proxy
    let proxy = Screencast::new().await?;

    // 2) Create a portal session
    let session = proxy.create_session().await?;

    // 3) Ask user to pick monitor/window
    proxy
        .select_sources(
            &session,
            CursorMode::Metadata,                 // or .Hidden / .Embedded
            SourceType::Monitor | SourceType::Window,
            true,                                 // allow multiple
            None,                                 // no explicit output node
            PersistMode::DoNot,                   // ask each time
        )
        .await?
        .response()?; // ensure user didn’t cancel

    // 4) Start the screencast session (shows portal UI etc)
    let start = proxy.start(&session, &WindowIdentifier::default()).await?;
    let start_response = start.response()?;
    let streams = start_response.streams();

    let first = streams
        .first()
        .ok_or("No screencast stream selected")?;

    let node_id = first.pipe_wire_node_id();
    let size = first.size();

    // 5) Open the PipeWire remote FD via the portal
    let remote_fd = proxy.open_pipe_wire_remote(&session).await?;

    // 6) Hand everything to the PipeWire loop
    let (width, height) = size.unwrap_or((1920, 1080));
    pipewire_capture_loop(remote_fd, node_id, tx, width as u32, height as u32)?;

    Ok(())
}

fn pipewire_capture_loop(
    remote_fd: i32,
    node_id: u32,
    tx: Sender<(usize, usize, Vec<u8>)>,
    width: u32,
    height: u32,
) -> CaptureResult<()> {
    pw::init(); // once per process ideally

    let main_loop = MainLoop::new(None)?;
    let context = Context::new(&main_loop)?;

    let owned_fd = unsafe { OwnedFd::from_raw_fd(remote_fd) };
    // Create core from existing fd (remote)
    let core = context
        .connect_fd(owned_fd, None)
        .map_err(|e| format!("Failed to connect PipeWire fd: {e}"))?;

    let stream = Stream::new(
        &core,
        "rust-remote-desktop",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )?;

    let listener_tx = tx.clone();

    let _listener = stream
        .add_local_listener_with_user_data(listener_tx)
        .process(move |stream_ref: &StreamRef, tx: &mut Sender<(usize, usize, Vec<u8>)>| {
            if let Some(mut buffer) = stream_ref.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    println!("Buffer has no data");
                    return;
                }

                let data = &mut datas[0];

                let available = data.chunk().size() as usize;

                if let Some(frame_bytes) = data.data() {
                    let full = frame_bytes.len();
                    let copy_len = available.min(full);

                    let expected = (width * height * 4) as usize;
                    let copy_len = copy_len.min(expected);

                    if copy_len > 0 {
                        let mut rgba = Vec::with_capacity(expected);

                        // PipeWire is giving us BGRA/BGRx; convert to RGBA as we copy.
                        for chunk in frame_bytes[..copy_len].chunks_exact(4) {
                            let b = chunk[0];
                            let g = chunk[1];
                            let r = chunk[2];
                            let a = chunk[3]; // if it's actually X (no alpha), this will just be padding

                            rgba.push(r);
                            rgba.push(g);
                            rgba.push(b);
                            rgba.push(a);
                        }
                    }
                }
            }
        })
        .register()?;

    // --- Connect the stream to the node we got from the portal ---
    //
    // No explicit format negotiation yet; we let PipeWire pick something
    // that matches the screen.
    let mut params: [&Pod; 0] = [];
    stream.connect(
        Direction::Input,           // we are *consuming* frames
        Some(node_id),              // the node selected via the portal
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut params,
    )?;

    // Start streaming
    stream.set_active(true)?;


    // Run the main loop (blocks until quit)
    main_loop.run();
    Ok(())
}
