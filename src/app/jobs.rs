use super::*;

impl PaintApp {
    pub(in crate::app) fn start_job(
        &mut self,
        ctx: &Context,
        job: impl FnOnce() -> JobResult + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.job = Some(rx);
        self.job_cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = job();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    pub(in crate::app) fn poll_job(&mut self) {
        if let Some(result) = self.job.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.job = None;
            if self.job_cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            match result {
                JobResult::Devices(Ok(devices)) => {
                    self.devices = devices;
                    self.device_index = 0;
                }
                JobResult::Image(Ok(image)) => {
                    self.insert_image(image);
                    self.dialog = None;
                }
                JobResult::Status(Ok(message)) => self.message = message,
                JobResult::Devices(Err(e))
                | JobResult::Image(Err(e))
                | JobResult::Status(Err(e)) => self.message = e,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canceled_capture_ignores_a_success_already_waiting_in_the_channel() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        let (sender, receiver) = std::sync::mpsc::channel();
        sender
            .send(JobResult::Image(Ok(RgbaImage::from_pixel(
                4,
                4,
                Rgba(BLACK),
            ))))
            .unwrap();
        app.job = Some(receiver);
        app.job_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);

        app.poll_job();

        assert!(app.job.is_none());
        assert!(app.doc.objects.is_empty());
        assert!(!app.doc.dirty());
    }
}
