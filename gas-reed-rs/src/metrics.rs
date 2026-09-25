use embeprom::{Counter, Gauge};

/// Gas meter metrics
pub mod meter_metrics {
    use defmt::info;
    use picoserve::extract::State;
    use picoserve::response::chunked::{ChunkWriter, Chunks, ChunksWritten};
    use picoserve::routing::MethodHandler;

    embeprom::metrics! {
        namespace = "gas_meter";

        /// Single meter count (tick), equals to 10L of burned gas
        meter_tick: Counter,

        /// Absolute meter value. Can be reset to match the value on the display. Grows with each count
        meter_absolute: Gauge,
    }

    pub struct MetricsResponse;
    impl Chunks for MetricsResponse {
        fn content_type(&self) -> &'static str {
            embeprom::CONTENT_TYPE
        }

        async fn write_chunks<W: picoserve::io::Write>(
            self,
            mut chunk_writer: ChunkWriter<W>,
        ) -> Result<ChunksWritten, W::Error> {
            let mut renderer = embeprom::Renderer::new();
            while let Some(line) = renderer
                .next_line()
                .expect("increase the Renderer line capacity")
            {
                info!("metrics line {}", line);
                chunk_writer.write_chunk(line.as_bytes()).await?;
            }
            chunk_writer.finalize().await
        }
    }


}