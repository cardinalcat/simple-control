use captrs::*;
use opencv::core::*;
use opencv::imgcodecs::*;
use opencv::videoio::*;
use opencv::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Screen,
    Camera,
}
impl Default for SourceType {
    fn default() -> Self {
        Self::Screen
    }
}

pub enum CaptureSource {
    Screen(Capturer),
    Camera(VideoCapture),
}
impl CaptureSource {
    pub fn new(source: SourceType, index: u32) -> Self {
        match source {
            SourceType::Screen => CaptureSource::Screen(Capturer::create(index).unwrap()),
            SourceType::Camera => CaptureSource::Camera(VideoCapture::create(index).unwrap()),
        }
    }
    pub fn grab_frame(&mut self) -> Option<Box<dyn InputFrame>>{
        match self {
            Self::Screen(scr) => scr.grab_frame(),
            Self::Camera(cam) => cam.grab_frame(),
        }
    }
}

pub trait Source {
    type Result;
    fn create(display: u32) -> Self::Result;
    fn grab_frame(&mut self) -> Option<Box<dyn InputFrame>>;
}
impl Source for Capturer {
    type Result = std::result::Result<Capturer, String>;
    fn create(display: u32) -> Self::Result {
        let capturer = Capturer::new(display as usize)?;
        Ok(capturer)
    }
    fn grab_frame(&mut self) -> Option<Box<dyn InputFrame>> {
        match self.capture_frame() {
            Ok(vec) => Some(Box::new((self.geometry().0, self.geometry().1, vec))),
            Err(_) => None,
        }
    }
}
impl Source for VideoCapture {
    type Result = opencv::Result<VideoCapture>;
    fn create(source: u32) -> Self::Result {
        let cam = VideoCapture::new(source as i32, CAP_ANY)?;
        if !VideoCapture::is_opened(&cam)? {
            panic!("unable to open camera");
        }
        Ok(cam)
    }
    fn grab_frame(&mut self) -> Option<Box<dyn InputFrame>> {
        let mut frame = match Mat::default() {
            Ok(f) => f,
            Err(_) => return None,
        };
        match self.read(&mut frame) {
            Ok(_) => (),
            Err(_) => return None,
        }
        Some(Box::new(frame))
    }
}

pub trait InputFrame {
    fn to_raw(&self) -> Vec<u8>;
    fn to_raw_compressed(&self, format: &str, quality: i32) -> Vec<u8>;
}
impl InputFrame for (u32, u32, Vec<Bgr8>) {
    fn to_raw(&self) -> Vec<u8> {
        let mut raw_vec = Vec::with_capacity(self.2.len() * 3);
        for pixel in self.2.iter() {
            raw_vec.push(pixel.b);
            raw_vec.push(pixel.g);
            raw_vec.push(pixel.r);
        }
        raw_vec
    }
    fn to_raw_compressed(&self, format: &str, quality: i32) -> Vec<u8> {
        let h = self.1;
        let w = self.0;
        let mut matrix = unsafe { Mat::new_rows_cols(h as i32, w as i32, CV_8UC3).unwrap() };
        for y in 0..matrix.rows() {
            for x in 0..matrix.cols() {
                let pixel = matrix.at_2d_mut::<Vec3b>(y, x).unwrap();
                let newpix = self.2.get((y as usize * w as usize) + x as usize).unwrap();
                pixel[0] = newpix.b;
                pixel[1] = newpix.g;
                pixel[2] = newpix.r;
            }
        }
        matrix.to_raw_compressed(format, quality)
    }
}
impl InputFrame for Mat {
    fn to_raw(&self) -> Vec<u8> {
        self.to_raw_compressed(".jpg", 100)
    }
    fn to_raw_compressed(&self, format: &str, quality: i32) -> Vec<u8> {
        let w = self.size().unwrap().width;
        let h = self.size().unwrap().height;
        let mut outbuf: core::Vector<u8> = core::Vector::with_capacity((w * h * 3) as usize);
        let mut params: core::Vector<i32> = core::Vector::new();
        params.push(IMWRITE_JPEG_QUALITY);
        params.push(quality);
        imencode(format, &self, &mut outbuf, &params).unwrap();
        outbuf.to_vec()
    }
}
pub trait OutputFrame {
    fn from_raw(vec: &[u8]) -> Self
        where Self: std::marker::Sized,
    {
        Self::from_raw_compressed(vec)
    }
    fn from_raw_compressed(vec: &[u8]) -> Self;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
}
pub trait DisplayFrame {
    fn u32frame(&self, vec: &mut Vec<u32>);
}
impl DisplayFrame for Mat {
    fn u32frame(&self, vec: &mut Vec<u32>){
        //let mut vec: Vec<u32> = Vec::with_capacity((self.width() * self.height()) as usize);
        for x in 0..self.width(){
            for y in 0..self.height(){
                let pix = &self.at_2d::<Vec3b>(x,y).unwrap();
                let pixel: [u8;4] = [pix[0], pix[1], pix[2], 0];
                vec.push(u32::from_be_bytes(pixel));
            }
        }
    }
}
impl OutputFrame for Mat {
    fn from_raw_compressed(vec: &[u8]) -> Self{
        let mut opencvvec: Vector<u8> = Vector::with_capacity(vec.len());
        for i in 0..vec.len(){
            opencvvec.push(vec[i]);
        }
        let mat = imdecode(&opencvvec, -1).unwrap();
        opencvvec.clear();
        std::mem::drop(opencvvec);
        mat
    }
    fn width(&self) -> i32{
        self.rows()
    }
    fn height(&self) -> i32{
        self.cols()
    }
}