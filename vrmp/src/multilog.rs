pub struct MultiLogger {
    loggers: Vec<Box<dyn log::Log>>,
}

impl MultiLogger {
    pub fn new(loggers: Vec<Box<dyn log::Log>>) -> Self {
        MultiLogger { loggers }
    }
}

impl log::Log for MultiLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.loggers.iter().any(|logger| logger.enabled(metadata))
    }

    fn log(&self, record: &log::Record) {
        self.loggers.iter().for_each(|logger| logger.log(record));
    }

    fn flush(&self) {
        self.loggers.iter().for_each(|logger| logger.flush());
    }
}
