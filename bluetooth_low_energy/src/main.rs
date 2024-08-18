mod bluetooth;

use bluetooth::message::BleMessage;
use bluetooth::BlePeripheral;

#[tokio::main]
async fn main() {
    env_logger::init();

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

    loop {
        if ble.is_subscribed().await {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Send a text message to the central device.
    ble.send_message("test".into()).await;

    // Asumming that the central device will send the same exact message back to the peripheral

    // Wait for the same message to be received.
    let message = ble.receive_message().await;
    if let BleMessage::Text(message) = message.convert_to_text().unwrap() {
        log::info!("{}", message);
    }

    // Open an image file.
    let image = tokio::fs::read("test_assets/test_image.jpg").await.unwrap();

    // Send the image file to the central device.
    ble.send_message(image.into()).await;

    // Wait for another message to be received.
    let message = ble.receive_message().await;
    if let BleMessage::Text(message) = message.convert_to_text().unwrap() {
        log::info!("{}", message);
    }

    // Stop the BLE peripheral engine.
    ble.stop_engine().await;
}
