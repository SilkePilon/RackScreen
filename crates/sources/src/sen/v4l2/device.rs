//! The V4L2 stateful decoder device, by raw ioctls: bitstream buffers in
//! (the OUTPUT queue), decoded pictures out (the CAPTURE queue), both
//! memory-mapped. Nothing here blocks for long: the device is opened
//! non-blocking and every wait has an end.
//!
//! The structs mirror `linux/videodev2.h` for 64-bit little-endian targets;
//! their sizes are part of the ioctl numbers and are checked at compile
//! time.

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use libc::{c_int, c_ulong};
use rackscreen_core::video::Planes;

use super::{
    Layout, Rect, Stamp, Stamps, CAPTURE_BUFFERS, OUTPUT_BUFFERS, OUTPUT_BYTES, STARVED_UNITS,
};

/// The Pi's decoder ("bcm2835-codec-decode").
const DEVICE: &str = "/dev/video10";
/// Another device to use instead.
const DEVICE_ENV: &str = "RACKSCREEN_SEN_V4L2_DEVICE";

/// The longest wait for the decoder to take a unit.
const OUTPUT_WAIT: Duration = Duration::from_secs(1);
/// A decoder that has not said what the pictures look like after this long
/// with all its input buffers full gets its picture buffers anyway (some
/// decode nothing, headers included, before they have them).
const EAGER_AFTER: Duration = Duration::from_millis(250);

const CAPTURE: u32 = 1;
const CAPTURE_MPLANE: u32 = 9;
const OUTPUT_MPLANE: u32 = 10;
const MEMORY_MMAP: u32 = 1;
const FIELD_NONE: u32 = 1;
const CAP_VIDEO_M2M_MPLANE: u32 = 0x4000;
const CAP_STREAMING: u32 = 0x0400_0000;
const CAP_DEVICE_CAPS: u32 = 0x8000_0000;
const EVENT_SOURCE_CHANGE: u32 = 5;
const SRC_CH_RESOLUTION: u32 = 1;
const SEL_TGT_COMPOSE: u32 = 0x100;
const BUF_FLAG_ERROR: u32 = 0x40;
/// The most buffers a queue can have.
const MAX_BUFFERS: u32 = 32;

const fn fourcc(c: &[u8; 4]) -> u32 {
    c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16 | (c[3] as u32) << 24
}
const PIX_H264: u32 = fourcc(b"H264");
const PIX_YU12: u32 = fourcc(b"YU12");

#[repr(C)]
#[derive(Default)]
struct Capability {
    driver: [u8; 16],
    card: [u8; 32],
    bus_info: [u8; 32],
    version: u32,
    capabilities: u32,
    device_caps: u32,
    reserved: [u32; 3],
}

#[repr(C)]
#[derive(Default)]
struct FmtDesc {
    index: u32,
    typ: u32,
    flags: u32,
    description: [u8; 32],
    pixelformat: u32,
    mbus_code: u32,
    reserved: [u32; 3],
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PlaneFmt {
    sizeimage: u32,
    bytesperline: u32,
    reserved: [u16; 6],
}

/// `v4l2_pix_format_mplane`
#[repr(C)]
#[derive(Default)]
struct PixMp {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    colorspace: u32,
    plane_fmt: [PlaneFmt; 8],
    num_planes: u8,
    flags: u8,
    ycbcr_enc: u8,
    quantization: u8,
    xfer_func: u8,
    reserved: [u8; 7],
}

/// `v4l2_format`, its union held as the multiplanar format it is here.
#[repr(C)]
#[derive(Default)]
struct Format {
    typ: u32,
    pad: u32,
    pix: PixMp,
    rest: [u32; 2],
}

#[repr(C)]
#[derive(Default)]
struct RequestBuffers {
    count: u32,
    typ: u32,
    memory: u32,
    capabilities: u32,
    flags: u8,
    reserved: [u8; 3],
}

/// `v4l2_plane`, its union held as the mmap offset.
#[repr(C)]
#[derive(Default)]
struct Plane {
    bytesused: u32,
    length: u32,
    mem_offset: u32,
    pad: u32,
    data_offset: u32,
    reserved: [u32; 11],
}

/// `v4l2_buffer`, its union held as the pointer to the planes.
#[repr(C)]
struct Buffer {
    index: u32,
    typ: u32,
    bytesused: u32,
    flags: u32,
    field: u32,
    timestamp: [i64; 2],
    timecode: [u32; 4],
    sequence: u32,
    memory: u32,
    planes: *mut Plane,
    length: u32,
    reserved2: u32,
    request_fd: i32,
}

impl Buffer {
    /// A buffer of one plane.
    fn new(typ: u32, index: u32, plane: &mut Plane) -> Buffer {
        Buffer {
            index,
            typ,
            bytesused: 0,
            flags: 0,
            field: FIELD_NONE,
            timestamp: [0; 2],
            timecode: [0; 4],
            sequence: 0,
            memory: MEMORY_MMAP,
            planes: plane,
            length: 1,
            reserved2: 0,
            request_fd: 0,
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct EventSubscription {
    typ: u32,
    id: u32,
    flags: u32,
    reserved: [u32; 5],
}

/// `v4l2_event`; `u[0]` is what changed in a source change.
#[repr(C)]
#[derive(Default)]
struct Event {
    typ: u32,
    pad: u32,
    u: [u32; 16],
    pending: u32,
    sequence: u32,
    timestamp: [i64; 2],
    id: u32,
    reserved: [u32; 8],
}

#[repr(C)]
#[derive(Default)]
struct Selection {
    typ: u32,
    target: u32,
    flags: u32,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    reserved: [u32; 9],
}

const _: () = {
    assert!(size_of::<Capability>() == 104);
    assert!(size_of::<FmtDesc>() == 64);
    assert!(size_of::<PixMp>() == 192);
    assert!(size_of::<Format>() == 208);
    assert!(size_of::<RequestBuffers>() == 20);
    assert!(size_of::<Plane>() == 64);
    assert!(size_of::<Buffer>() == 88);
    assert!(size_of::<EventSubscription>() == 32);
    assert!(size_of::<Event>() == 136);
    assert!(size_of::<Selection>() == 64);
};

const IOC_WRITE: c_ulong = 1;
const IOC_READ: c_ulong = 2;

/// An ioctl number of the video ('V') family with `T` as its argument.
const fn ioc<T>(dir: c_ulong, nr: c_ulong) -> c_ulong {
    dir << 30 | (size_of::<T>() as c_ulong) << 16 | (b'V' as c_ulong) << 8 | nr
}

const VIDIOC_QUERYCAP: c_ulong = ioc::<Capability>(IOC_READ, 0);
const VIDIOC_ENUM_FMT: c_ulong = ioc::<FmtDesc>(IOC_READ | IOC_WRITE, 2);
const VIDIOC_G_FMT: c_ulong = ioc::<Format>(IOC_READ | IOC_WRITE, 4);
const VIDIOC_S_FMT: c_ulong = ioc::<Format>(IOC_READ | IOC_WRITE, 5);
const VIDIOC_REQBUFS: c_ulong = ioc::<RequestBuffers>(IOC_READ | IOC_WRITE, 8);
const VIDIOC_QUERYBUF: c_ulong = ioc::<Buffer>(IOC_READ | IOC_WRITE, 9);
const VIDIOC_QBUF: c_ulong = ioc::<Buffer>(IOC_READ | IOC_WRITE, 15);
const VIDIOC_DQBUF: c_ulong = ioc::<Buffer>(IOC_READ | IOC_WRITE, 17);
const VIDIOC_STREAMON: c_ulong = ioc::<c_int>(IOC_WRITE, 18);
const VIDIOC_STREAMOFF: c_ulong = ioc::<c_int>(IOC_WRITE, 19);
const VIDIOC_DQEVENT: c_ulong = ioc::<Event>(IOC_READ, 89);
const VIDIOC_SUBSCRIBE_EVENT: c_ulong = ioc::<EventSubscription>(IOC_WRITE, 90);
const VIDIOC_G_SELECTION: c_ulong = ioc::<Selection>(IOC_READ | IOC_WRITE, 94);

/// One ioctl, started again when a signal interrupted it.
fn ioctl<T>(fd: RawFd, request: c_ulong, arg: &mut T) -> io::Result<()> {
    // the number carries the size of its argument: never hand the kernel
    // a smaller one
    if (request >> 16) & 0x3FFF != size_of::<T>() as c_ulong {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    loop {
        // SAFETY: `arg` is a live, exclusive `T` of the size the request
        // number names, which is all the kernel reads or writes (the plane
        // a `Buffer` points to is kept alive by the callers).
        if unsafe { libc::ioctl(fd, request, arg as *mut T) } == 0 {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// One memory-mapped buffer of a queue.
struct Mapping {
    ptr: NonNull<u8>,
    len: usize,
}

// SAFETY: the mapping is plain memory owned by this value alone.
unsafe impl Send for Mapping {}

impl Mapping {
    fn new(fd: RawFd, typ: u32, index: u32) -> Result<Mapping> {
        let mut plane = Plane::default();
        let mut buf = Buffer::new(typ, index, &mut plane);
        ioctl(fd, VIDIOC_QUERYBUF, &mut buf).context("VIDIOC_QUERYBUF")?;
        let len = plane.length as usize;
        if len == 0 {
            bail!("buffer {index} is empty");
        }
        // SAFETY: a fresh shared mapping of `len` bytes of the device's
        // buffer, at an address the kernel picks.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                plane.mem_offset as libc::off_t,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error()).context("map a buffer");
        }
        let ptr = NonNull::new(ptr.cast::<u8>()).ok_or_else(|| anyhow!("map a buffer"))?;
        Ok(Mapping { ptr, len })
    }

    /// The buffer's bytes. Only to be looked at while the buffer is not
    /// queued (the decoder writes to queued ones).
    fn bytes(&self) -> &[u8] {
        // SAFETY: `ptr` maps `len` readable bytes for as long as `self` lives.
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: as `bytes`; the mapping is writable and `self` is exclusive.
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: exactly the region `mmap` gave; nothing borrows it any more.
        unsafe {
            libc::munmap(self.ptr.as_ptr().cast(), self.len);
        }
    }
}

/// The picture queue once the decoder said what the pictures look like.
struct Capture {
    bufs: Vec<Mapping>,
    layout: Layout,
}

/// A buffer the decoder handed back.
struct Done {
    index: u32,
    flags: u32,
    bytesused: u32,
    stamp: (i64, i64),
}

pub struct Decoder {
    /// Bitstream buffers, and the ones of them that are not queued.
    out: Vec<Mapping>,
    out_free: Vec<u32>,
    /// The smallest bitstream buffer: the largest unit that fits.
    out_bytes: usize,
    cap: Option<Capture>,
    stamps: Stamps,
    /// Units queued since the last picture came out.
    starved: u32,
    name: String,
    /// Last: the mappings above go before the device is closed.
    file: File,
}

impl Decoder {
    /// Open the device and start its bitstream queue. The picture queue
    /// follows once the decoder has seen the stream's headers.
    pub fn open() -> Result<Decoder> {
        let path = std::env::var_os(DEVICE_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEVICE));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
            .with_context(|| format!("open {}", path.display()))?;
        let fd = file.as_raw_fd();

        let mut caps = Capability::default();
        ioctl(fd, VIDIOC_QUERYCAP, &mut caps).context("VIDIOC_QUERYCAP")?;
        let has = if caps.capabilities & CAP_DEVICE_CAPS != 0 {
            caps.device_caps
        } else {
            caps.capabilities
        };
        if has & CAP_VIDEO_M2M_MPLANE == 0 || has & CAP_STREAMING == 0 {
            bail!("{} is not a memory-to-memory video device", path.display());
        }
        let driver = caps.driver.split(|b| *b == 0).next().unwrap_or(&[]);
        let name = format!("{}, {}", path.display(), String::from_utf8_lossy(driver));

        let takes_h264 = (0..64).any(|index| {
            let mut desc = FmtDesc {
                index,
                typ: OUTPUT_MPLANE,
                ..FmtDesc::default()
            };
            ioctl(fd, VIDIOC_ENUM_FMT, &mut desc).is_ok() && desc.pixelformat == PIX_H264
        });
        if !takes_h264 {
            bail!("{name} does not decode H.264");
        }

        // the size is a first guess (the stream asked for is 360p); the
        // decoder reports the real one from the stream's headers
        let mut fmt = Format {
            typ: OUTPUT_MPLANE,
            ..Format::default()
        };
        fmt.pix.width = 640;
        fmt.pix.height = 360;
        fmt.pix.pixelformat = PIX_H264;
        fmt.pix.field = FIELD_NONE;
        fmt.pix.num_planes = 1;
        fmt.pix.plane_fmt[0].sizeimage = OUTPUT_BYTES;
        ioctl(fd, VIDIOC_S_FMT, &mut fmt).context("set the H.264 format")?;
        if fmt.pix.pixelformat != PIX_H264 {
            bail!("{name} does not take the H.264 format");
        }

        let mut sub = EventSubscription {
            typ: EVENT_SOURCE_CHANGE,
            ..EventSubscription::default()
        };
        ioctl(fd, VIDIOC_SUBSCRIBE_EVENT, &mut sub).context("subscribe to source changes")?;

        let count = request_buffers(fd, OUTPUT_MPLANE, OUTPUT_BUFFERS)
            .context("request the bitstream buffers")?;
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            out.push(Mapping::new(fd, OUTPUT_MPLANE, index)?);
        }
        let out_bytes = out.iter().map(|m| m.len).min().unwrap_or(0);
        stream(fd, VIDIOC_STREAMON, OUTPUT_MPLANE).context("start the bitstream queue")?;
        Ok(Decoder {
            out_free: (0..count).collect(),
            out,
            out_bytes,
            cap: None,
            stamps: Stamps::default(),
            starved: 0,
            name,
            file,
        })
    }

    /// The device and its driver, for the log.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The next unit does not follow the ones queued so far: their
    /// pictures are not handed out any more.
    pub fn reset(&mut self) {
        self.stamps.reset();
    }

    /// Queue one access unit and hand out the pictures that are ready (of
    /// earlier units: the decoder works while the caller waits for the next
    /// unit's time). The planes are the decoder's own buffer, valid during
    /// the call. `Ok(false)`: the unit was left out. An error means the
    /// decoder is of no use any more.
    pub fn decode(
        &mut self,
        data: &[u8],
        pts: Option<u64>,
        on_picture: &mut dyn FnMut(Option<u64>, &Planes),
    ) -> Result<bool> {
        self.service(on_picture)?;
        // an empty buffer would mean the end of the stream
        if data.is_empty() || data.len() > self.out_bytes {
            return Ok(false);
        }
        let index = self.free_output(on_picture)?;
        let Some(buf) = self.out.get_mut(index as usize) else {
            bail!("the decoder returned a buffer it does not have");
        };
        buf.bytes_mut()[..data.len()].copy_from_slice(data);
        let stamp = self.stamps.push(pts);
        if let Err(e) = self.qbuf(OUTPUT_MPLANE, index, data.len() as u32, stamp) {
            self.out_free.push(index);
            return Err(e).context("queue a unit");
        }
        self.starved += 1;
        if self.starved > STARVED_UNITS {
            bail!("no picture for {STARVED_UNITS} units");
        }
        Ok(true)
    }

    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// A bitstream buffer to fill, waiting a bounded time for the decoder
    /// to hand one back.
    fn free_output(&mut self, on_picture: &mut dyn FnMut(Option<u64>, &Planes)) -> Result<u32> {
        let start = Instant::now();
        loop {
            if let Some(index) = self.out_free.pop() {
                return Ok(index);
            }
            let waited = start.elapsed();
            if waited > OUTPUT_WAIT {
                bail!("the decoder takes no more units");
            }
            if self.cap.is_none() && waited > EAGER_AFTER {
                self.configure_capture()?;
            }
            let mut pfd = libc::pollfd {
                fd: self.fd(),
                events: libc::POLLIN | libc::POLLOUT | libc::POLLPRI,
                revents: 0,
            };
            // SAFETY: one valid pollfd; the wait ends after 20 ms.
            let ready = unsafe { libc::poll(&mut pfd, 1, 20) };
            let wanted = libc::POLLIN | libc::POLLOUT | libc::POLLPRI;
            if ready < 0 || (ready > 0 && pfd.revents & wanted == 0) {
                // an error state that poll reports at once: do not spin
                std::thread::sleep(Duration::from_millis(2));
            }
            self.service(on_picture)?;
        }
    }

    /// Everything the decoder has for us, without waiting: a new picture
    /// format, bitstream buffers it is done with, decoded pictures.
    fn service(&mut self, on_picture: &mut dyn FnMut(Option<u64>, &Planes)) -> Result<()> {
        let mut changed = false;
        for _ in 0..16 {
            let mut ev = Event::default();
            if ioctl(self.fd(), VIDIOC_DQEVENT, &mut ev).is_err() {
                break;
            }
            changed |= ev.typ == EVENT_SOURCE_CHANGE && ev.u[0] & SRC_CH_RESOLUTION != 0;
        }
        if changed {
            self.configure_capture()?;
        }

        while self.out_free.len() < self.out.len() {
            match self
                .dqbuf(OUTPUT_MPLANE)
                .context("take back a unit's buffer")?
            {
                Some(done) if (done.index as usize) < self.out.len() => {
                    if !self.out_free.contains(&done.index) {
                        self.out_free.push(done.index);
                    }
                }
                Some(_) => bail!("the decoder returned a buffer it does not have"),
                None => break,
            }
        }

        let rounds = self.cap.as_ref().map_or(0, |c| 2 * c.bufs.len());
        for _ in 0..rounds {
            let done = match self.dqbuf(CAPTURE_MPLANE) {
                Ok(Some(done)) => done,
                Ok(None) => break,
                // the end of the pictures in the old format
                Err(e) if e.raw_os_error() == Some(libc::EPIPE) => {
                    self.configure_capture()?;
                    break;
                }
                Err(e) => return Err(e).context("take a picture"),
            };
            self.starved = 0;
            let stamp = self.stamps.take(done.stamp.0, done.stamp.1);
            let good = done.flags & BUF_FLAG_ERROR == 0 && done.bytesused > 0;
            if let (true, Stamp::Fresh(pts), Some(cap)) = (good, stamp, &self.cap) {
                let planes = cap
                    .bufs
                    .get(done.index as usize)
                    .and_then(|buf| cap.layout.planes(buf.bytes()));
                if let Some(planes) = planes {
                    on_picture(pts, &planes);
                }
            }
            // back to the decoder at once: it must never run out
            self.qbuf(CAPTURE_MPLANE, done.index, 0, (0, 0))
                .context("give a picture buffer back")?;
        }
        Ok(())
    }

    /// (Re)build the picture queue for the format the decoder reports.
    fn configure_capture(&mut self) -> Result<()> {
        self.release_capture();
        let fd = self.fd();
        let mut fmt = Format {
            typ: CAPTURE_MPLANE,
            ..Format::default()
        };
        ioctl(fd, VIDIOC_G_FMT, &mut fmt).context("get the picture format")?;
        if fmt.pix.pixelformat != PIX_YU12 {
            // ask for I420 with the visible size: the driver pads it itself
            let vis = visible(fd);
            fmt.pix.pixelformat = PIX_YU12;
            fmt.pix.width = vis.map_or(fmt.pix.width, |r| r.width);
            fmt.pix.height = vis.map_or(fmt.pix.height, |r| r.height);
            fmt.pix.num_planes = 1;
            fmt.pix.plane_fmt = [PlaneFmt::default(); 8];
            ioctl(fd, VIDIOC_S_FMT, &mut fmt).context("set the picture format")?;
        }
        if fmt.pix.pixelformat != PIX_YU12 || fmt.pix.num_planes != 1 {
            bail!("the decoder has no single-plane I420 pictures");
        }
        let line = fmt.pix.plane_fmt[0].bytesperline;
        let layout = Layout::i420(fmt.pix.width, fmt.pix.height, line, visible(fd))
            .or_else(|| Layout::i420(fmt.pix.width, fmt.pix.height, line, None))
            .ok_or_else(|| {
                anyhow!(
                    "a picture format of {}x{}, {line} bytes a line",
                    fmt.pix.width,
                    fmt.pix.height
                )
            })?;

        let count = request_buffers(fd, CAPTURE_MPLANE, CAPTURE_BUFFERS)
            .context("request the picture buffers")?;
        let mut bufs = Vec::with_capacity(count as usize);
        for index in 0..count {
            let buf = Mapping::new(fd, CAPTURE_MPLANE, index)?;
            if buf.len < layout.need {
                bail!(
                    "picture buffers of {} bytes for pictures of {}",
                    buf.len,
                    layout.need
                );
            }
            bufs.push(buf);
        }
        tracing::debug!(
            "sen: v4l2 pictures {}x{} in {}x{}, {line} bytes a line, {count} buffers",
            layout.width,
            layout.height,
            fmt.pix.width,
            fmt.pix.height
        );
        // set before queueing, so a failure below still frees the buffers
        self.cap = Some(Capture { bufs, layout });
        for index in 0..count {
            self.qbuf(CAPTURE_MPLANE, index, 0, (0, 0))
                .context("queue a picture buffer")?;
        }
        stream(fd, VIDIOC_STREAMON, CAPTURE_MPLANE).context("start the picture queue")
    }

    /// Stop the picture queue and free its buffers.
    fn release_capture(&mut self) {
        let fd = self.fd();
        let _ = stream(fd, VIDIOC_STREAMOFF, CAPTURE_MPLANE);
        // unmapped first: the driver does not free mapped buffers
        self.cap = None;
        let _ = request_buffers(fd, CAPTURE_MPLANE, 0);
    }

    fn qbuf(&self, typ: u32, index: u32, bytesused: u32, stamp: (i64, i64)) -> io::Result<()> {
        let mut plane = Plane {
            bytesused,
            ..Plane::default()
        };
        let mut buf = Buffer::new(typ, index, &mut plane);
        buf.timestamp = [stamp.0, stamp.1];
        ioctl(self.fd(), VIDIOC_QBUF, &mut buf)
    }

    /// The next buffer the decoder is done with, if there is one.
    fn dqbuf(&self, typ: u32) -> io::Result<Option<Done>> {
        let mut plane = Plane::default();
        let mut buf = Buffer::new(typ, 0, &mut plane);
        match ioctl(self.fd(), VIDIOC_DQBUF, &mut buf) {
            Ok(()) => Ok(Some(Done {
                index: buf.index,
                flags: buf.flags,
                bytesused: plane.bytesused,
                stamp: (buf.timestamp[0], buf.timestamp[1]),
            })),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // Stopping the queues drops what is still in the decoder; nobody
        // wants those pictures. The buffers are unmapped and the device is
        // closed as the fields go.
        self.release_capture();
        let fd = self.fd();
        let _ = stream(fd, VIDIOC_STREAMOFF, OUTPUT_MPLANE);
        self.out.clear();
        let _ = request_buffers(fd, OUTPUT_MPLANE, 0);
    }
}

fn stream(fd: RawFd, request: c_ulong, typ: u32) -> io::Result<()> {
    let mut typ = typ as c_int;
    ioctl(fd, request, &mut typ)
}

/// Ask for `count` buffers on a queue; the driver says how many it gives.
/// A count of zero frees them.
fn request_buffers(fd: RawFd, typ: u32, count: u32) -> Result<u32> {
    let mut req = RequestBuffers {
        count,
        typ,
        memory: MEMORY_MMAP,
        ..RequestBuffers::default()
    };
    ioctl(fd, VIDIOC_REQBUFS, &mut req)?;
    if count > 0 && (req.count == 0 || req.count > MAX_BUFFERS) {
        bail!("the driver gives {} buffers", req.count);
    }
    Ok(req.count)
}

/// The visible rectangle of the decoded pictures.
fn visible(fd: RawFd) -> Option<Rect> {
    // the single-planar type is the documented one; some drivers want the
    // queue's own
    [CAPTURE, CAPTURE_MPLANE].into_iter().find_map(|typ| {
        let mut sel = Selection {
            typ,
            target: SEL_TGT_COMPOSE,
            ..Selection::default()
        };
        ioctl(fd, VIDIOC_G_SELECTION, &mut sel).ok()?;
        Some(Rect {
            left: u32::try_from(sel.left).ok()?,
            top: u32::try_from(sel.top).ok()?,
            width: sel.width,
            height: sel.height,
        })
        .filter(|r| r.width > 0 && r.height > 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ioctl_numbers_are_the_kernels() {
        // from linux/videodev2.h on a 64-bit kernel
        assert_eq!(VIDIOC_QUERYCAP, 0x8068_5600);
        assert_eq!(VIDIOC_ENUM_FMT, 0xc040_5602);
        assert_eq!(VIDIOC_G_FMT, 0xc0d0_5604);
        assert_eq!(VIDIOC_S_FMT, 0xc0d0_5605);
        assert_eq!(VIDIOC_REQBUFS, 0xc014_5608);
        assert_eq!(VIDIOC_QUERYBUF, 0xc058_5609);
        assert_eq!(VIDIOC_QBUF, 0xc058_560f);
        assert_eq!(VIDIOC_DQBUF, 0xc058_5611);
        assert_eq!(VIDIOC_STREAMON, 0x4004_5612);
        assert_eq!(VIDIOC_STREAMOFF, 0x4004_5613);
        assert_eq!(VIDIOC_DQEVENT, 0x8088_5659);
        assert_eq!(VIDIOC_SUBSCRIBE_EVENT, 0x4020_565a);
        assert_eq!(VIDIOC_G_SELECTION, 0xc040_565e);
        assert_eq!(PIX_H264, 0x3436_3248);
        assert_eq!(PIX_YU12, 0x3231_5559);
    }

    #[test]
    fn the_struct_fields_lie_where_the_kernel_has_them() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(Capability, capabilities), 84);
        assert_eq!(offset_of!(Capability, device_caps), 88);
        assert_eq!(offset_of!(FmtDesc, pixelformat), 44);
        assert_eq!(offset_of!(Format, pix), 8);
        assert_eq!(offset_of!(PixMp, plane_fmt), 20);
        assert_eq!(offset_of!(PixMp, num_planes), 180);
        assert_eq!(offset_of!(Buffer, timestamp), 24);
        assert_eq!(offset_of!(Buffer, sequence), 56);
        assert_eq!(offset_of!(Buffer, memory), 60);
        assert_eq!(offset_of!(Buffer, planes), 64);
        assert_eq!(offset_of!(Buffer, length), 72);
        assert_eq!(offset_of!(Plane, mem_offset), 8);
        assert_eq!(offset_of!(Plane, data_offset), 16);
        assert_eq!(offset_of!(Event, u), 8);
        assert_eq!(offset_of!(Event, pending), 72);
        assert_eq!(offset_of!(Event, id), 96);
    }

    #[test]
    fn an_ioctl_with_the_wrong_argument_is_refused_before_the_kernel() {
        let mut small = 0u8;
        let e = ioctl(-1, VIDIOC_QBUF, &mut small).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput);
        // and a bad descriptor is an error, not a panic
        let mut typ: c_int = 0;
        assert!(ioctl(-1, VIDIOC_STREAMON, &mut typ).is_err());
    }

    #[test]
    fn a_file_that_is_no_video_device_takes_no_ioctl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("video");
        std::fs::write(&path, b"").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut caps = Capability::default();
        assert!(ioctl(file.as_raw_fd(), VIDIOC_QUERYCAP, &mut caps).is_err());
        assert!(visible(file.as_raw_fd()).is_none());
        assert!(request_buffers(file.as_raw_fd(), OUTPUT_MPLANE, 3).is_err());
    }
}
