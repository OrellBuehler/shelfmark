#![allow(dead_code)]

fn atom(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 8);
    out.extend_from_slice(&(payload.len() as u32 + 8).to_be_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(payload);
    out
}

fn full(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut p = vec![0u8; 4];
    p.extend_from_slice(payload);
    atom(name, &p)
}

fn data(kind: u32, payload: &[u8]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&kind.to_be_bytes());
    p.extend_from_slice(&[0u8; 4]);
    p.extend_from_slice(payload);
    atom(b"data", &p)
}

#[derive(Clone, Default)]
pub struct M4b {
    pub items: Vec<(Vec<u8>, Vec<u8>)>,
    pub freeform: Vec<(String, String, String)>,
    pub cover: Option<Vec<u8>>,
    pub track: Option<u16>,
    pub duration_ms: u32,
}

impl M4b {
    pub fn new(duration_ms: u32) -> Self {
        M4b {
            duration_ms,
            ..Default::default()
        }
    }

    pub fn text(mut self, fourcc: &[u8; 4], value: &str) -> Self {
        self.items.push((fourcc.to_vec(), value.as_bytes().to_vec()));
        self
    }

    pub fn free(mut self, mean: &str, name: &str, value: &str) -> Self {
        self.freeform.push((mean.into(), name.into(), value.into()));
        self
    }

    pub fn cover(mut self, jpeg: Vec<u8>) -> Self {
        self.cover = Some(jpeg);
        self
    }

    pub fn track(mut self, n: u16) -> Self {
        self.track = Some(n);
        self
    }

    pub fn build(&self) -> Vec<u8> {
        let mut ilst = Vec::new();
        for (name, value) in &self.items {
            let name: [u8; 4] = name.as_slice().try_into().unwrap();
            ilst.extend(atom(&name, &data(1, value)));
        }
        for (mean, name, value) in &self.freeform {
            let mut p = full(b"mean", mean.as_bytes());
            p.extend(full(b"name", name.as_bytes()));
            p.extend(data(1, value.as_bytes()));
            ilst.extend(atom(b"----", &p));
        }
        if let Some(n) = self.track {
            let mut p = vec![0u8, 0];
            p.extend_from_slice(&n.to_be_bytes());
            p.extend_from_slice(&[0u8; 4]);
            ilst.extend(atom(b"trkn", &data(0, &p)));
        }
        if let Some(c) = &self.cover {
            ilst.extend(atom(b"covr", &data(13, c)));
        }

        let mut hdlr = vec![0u8; 4];
        hdlr.extend_from_slice(b"mdir");
        hdlr.extend_from_slice(b"appl");
        hdlr.extend_from_slice(&[0u8; 9]);
        let mut meta = full(b"hdlr", &hdlr);
        meta.extend(atom(b"ilst", &ilst));
        let udta = atom(b"udta", &full(b"meta", &meta));

        let mut mvhd = vec![0u8; 8];
        mvhd.extend_from_slice(&1000u32.to_be_bytes());
        mvhd.extend_from_slice(&self.duration_ms.to_be_bytes());
        mvhd.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        mvhd.extend_from_slice(&0x0100u16.to_be_bytes());
        mvhd.extend_from_slice(&[0u8; 10]);
        mvhd.extend_from_slice(&[0u8; 36]);
        mvhd.extend_from_slice(&[0u8; 24]);
        mvhd.extend_from_slice(&2u32.to_be_bytes());

        let mut mdhd = vec![0u8; 8];
        mdhd.extend_from_slice(&1000u32.to_be_bytes());
        mdhd.extend_from_slice(&self.duration_ms.to_be_bytes());
        mdhd.extend_from_slice(&[0x55, 0xc4, 0, 0]);

        let mut soun = vec![0u8; 4];
        soun.extend_from_slice(b"soun");
        soun.extend_from_slice(&[0u8; 13]);

        let mut mp4a = vec![0u8; 6];
        mp4a.extend_from_slice(&1u16.to_be_bytes());
        mp4a.extend_from_slice(&[0u8; 8]);
        mp4a.extend_from_slice(&2u16.to_be_bytes());
        mp4a.extend_from_slice(&16u16.to_be_bytes());
        mp4a.extend_from_slice(&[0u8; 4]);
        mp4a.extend_from_slice(&(44100u32 << 16).to_be_bytes());
        let mut stsd = 1u32.to_be_bytes().to_vec();
        stsd.extend(atom(b"mp4a", &mp4a));

        let mut stts = 1u32.to_be_bytes().to_vec();
        stts.extend_from_slice(&1u32.to_be_bytes());
        stts.extend_from_slice(&self.duration_ms.to_be_bytes());

        let mut stbl = full(b"stsd", &stsd);
        stbl.extend(full(b"stts", &stts));
        stbl.extend(full(b"stsc", &0u32.to_be_bytes()));
        stbl.extend(full(b"stsz", &[0u8; 8]));
        stbl.extend(full(b"stco", &0u32.to_be_bytes()));

        let mut mdia = full(b"mdhd", &mdhd);
        mdia.extend(full(b"hdlr", &soun));
        mdia.extend(atom(b"minf", &atom(b"stbl", &stbl)));

        let mut tkhd = vec![0u8; 8];
        tkhd.extend_from_slice(&1u32.to_be_bytes());
        tkhd.extend_from_slice(&[0u8; 4]);
        tkhd.extend_from_slice(&self.duration_ms.to_be_bytes());
        tkhd.extend_from_slice(&[0u8; 60]);
        let mut trak = full(b"tkhd", &tkhd);
        trak.extend(atom(b"mdia", &mdia));

        let mut moov = full(b"mvhd", &mvhd);
        moov.extend(atom(b"trak", &trak));
        moov.extend(udta);

        let mut ftyp = b"M4B ".to_vec();
        ftyp.extend_from_slice(&[0u8; 4]);
        ftyp.extend_from_slice(b"isomM4B mp42");
        let mut out = atom(b"ftyp", &ftyp);
        out.extend(atom(b"moov", &moov));
        out.extend(atom(b"mdat", &[0u8; 64]));
        out
    }
}

fn synchsafe(n: u32) -> [u8; 4] {
    [
        ((n >> 21) & 0x7f) as u8,
        ((n >> 14) & 0x7f) as u8,
        ((n >> 7) & 0x7f) as u8,
        (n & 0x7f) as u8,
    ]
}

fn frame(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(payload);
    out
}

#[derive(Clone, Default)]
pub struct Mp3 {
    pub frames: Vec<(Vec<u8>, String)>,
    pub user: Vec<(String, String)>,
    pub seconds: u32,
}

impl Mp3 {
    pub fn new(seconds: u32) -> Self {
        Mp3 {
            seconds,
            ..Default::default()
        }
    }

    pub fn text(mut self, id: &[u8; 4], value: &str) -> Self {
        self.frames.push((id.to_vec(), value.into()));
        self
    }

    pub fn user(mut self, desc: &str, value: &str) -> Self {
        self.user.push((desc.into(), value.into()));
        self
    }

    pub fn build(&self) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, value) in &self.frames {
            let id: [u8; 4] = id.as_slice().try_into().unwrap();
            let mut p = vec![3u8];
            p.extend_from_slice(value.as_bytes());
            body.extend(frame(&id, &p));
        }
        for (desc, value) in &self.user {
            let mut p = vec![3u8];
            p.extend_from_slice(desc.as_bytes());
            p.push(0);
            p.extend_from_slice(value.as_bytes());
            body.extend(frame(b"TXXX", &p));
        }
        let mut out = b"ID3".to_vec();
        out.extend_from_slice(&[3, 0, 0]);
        out.extend_from_slice(&synchsafe(body.len() as u32));
        out.extend(body);
        let frames = (self.seconds as usize * 44100).div_ceil(1152).max(4);
        for _ in 0..frames {
            let mut f = vec![0xff, 0xfb, 0x90, 0x64];
            f.resize(417, 0);
            out.extend(f);
        }
        out
    }
}

pub fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb([200, 40, 40]));
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
    img.write_with_encoder(enc).unwrap();
    out
}
