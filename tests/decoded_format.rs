//! The registry decoder reports the size of the frame it last returned
//! (oxideav-core `Decoder::output_video_dimensions` /
//! `output_pixel_format`): an odd size, then a new size at a key frame.
//! Frames are decoded in `receive_frame`, so a queued key frame changes
//! nothing until it is returned.
//!
//! `decoded_format/*.ivf` are three-frame libvpx encodes of FFmpeg
//! `testsrc`.

#![cfg(feature = "registry")]

use oxideav_core::{CodecId, CodecParameters, Error, Frame, Packet, PixelFormat, TimeBase};

/// Every frame of both streams, in order, with its size.
fn frames() -> Vec<(Vec<u8>, (u32, u32))> {
    let streams: [(&[u8], (u32, u32)); 2] = [
        (include_bytes!("decoded_format/a_33x17.ivf"), (33, 17)),
        (include_bytes!("decoded_format/b_48x32.ivf"), (48, 32)),
    ];
    let mut out = Vec::new();
    for (file, size) in streams {
        // IVF: a file header of the length at bytes 6..8, then a 12-byte
        // header (size, pts) before each frame.
        let mut at = usize::from(u16::from_le_bytes([file[6], file[7]]));
        while at + 12 <= file.len() {
            let len = u32::from_le_bytes(file[at..at + 4].try_into().unwrap()) as usize;
            out.push((file[at + 12..at + 12 + len].to_vec(), size));
            at += 12 + len;
        }
    }
    assert_eq!(out.len(), 6);
    out
}

#[test]
fn each_frame_reports_its_own_size() {
    let mut dec = oxideav_vp8::decoder::make_decoder(&CodecParameters::video(CodecId::new("vp8")))
        .expect("decoder");
    let mut last = None;
    for (at, (data, (w, h))) in frames().into_iter().enumerate() {
        dec.send_packet(&Packet::new(0, TimeBase::new(1, 25), data))
            .expect("send");
        if last.is_some() {
            assert_eq!(
                dec.output_video_dimensions(),
                last,
                "frame {at}: before it is returned"
            );
        }
        let frame = match dec.receive_frame() {
            Ok(Frame::Video(frame)) => frame,
            other => panic!("frame {at}: {other:?}"),
        };
        assert_eq!(dec.output_video_dimensions(), Some((w, h)), "frame {at}");
        assert_eq!(
            dec.output_pixel_format(),
            Some(PixelFormat::Yuv420P),
            "frame {at}"
        );
        let planes = frame.image_planes();
        assert_eq!(planes.len(), 3, "frame {at}: planes");
        for (i, plane) in planes.iter().enumerate() {
            let (pw, ph) = PixelFormat::Yuv420P.plane_dimensions(i, w, h).unwrap();
            assert_eq!(plane.stride, pw as usize, "frame {at}: plane {i} stride");
            assert_eq!(
                plane.data.len(),
                (pw * ph) as usize,
                "frame {at}: plane {i} rows"
            );
        }
        assert!(
            matches!(dec.receive_frame(), Err(Error::NeedMore)),
            "frame {at}: one frame per packet"
        );
        last = Some((w, h));
    }
}
