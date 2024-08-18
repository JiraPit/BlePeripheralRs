#[cfg(test)]
mod bluetooth_test {
    use super::super::BlePeripheral;

    #[tokio::test]
    async fn full_test() {
        // Check if the user wants to run this test
        let should_run = std::env::var("TEST_BLUETOOTH").unwrap_or("0".to_string());
        if should_run != "1" {
            return;
        }

        // Create a new BLE peripheral.
        let mut ble = BlePeripheral::new(Some("TESTER".to_string()))
            .await
            .unwrap();

        // Start the BLE peripheral engine.
        ble.start_engine().await.unwrap();

        // Send a message to the central device.
        ble.send_message("test".into()).await;

        // Asumming that the central device will send the same exact message back to the peripheral

        // Wait for the same message to be received.
        let message = ble.receive_message().await;
        assert_eq!(message.as_string(), "test");

        // Stop the BLE peripheral engine.
        ble.stop_engine().await;
    }
}
