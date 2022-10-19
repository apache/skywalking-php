use phper::sys;

pub struct ExceptionFrame {
    #[allow(dead_code)]
    not_matter: i32,
}

impl ExceptionFrame {
    pub fn new() -> Self {
        unsafe {
            sys::zend_exception_save();
        }
        ExceptionFrame { not_matter: 0 }
    }
}

impl Drop for ExceptionFrame {
    fn drop(&mut self) {
        unsafe {
            sys::zend_exception_restore();
        }
    }
}
