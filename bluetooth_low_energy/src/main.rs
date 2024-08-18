mod bluetooth;

use bluetooth::message::BleMessage;
use bluetooth::BlePeripheral;
use std::vec::Vec;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Create a new BLE peripheral.
    let mut ble = BlePeripheral::new(Some("TESTER".to_string()))
        .await
        .unwrap();

    // Start the BLE peripheral engine.
    ble.start_engine().await.unwrap();

    // Wait for the central device to subscribe to the peripheral.
    loop {
        if ble.is_subscribed().await {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Wait for the central device to send the Ready message.
    loop {
        let message = ble.receive_message().await;
        if let BleMessage::Text(message) = message.convert_to_text().unwrap() {
            if message == "Ready" {
                break;
            }
        }
    }

    let mut time_records: Vec<tokio::time::Duration> = Vec::new();

    for i in 0..10 {
        // Open an image file.
        let image = match i % 2 {
            0 => tokio::fs::read("test_assets/test_image1.png")
                .await
                .unwrap(),
            1 => tokio::fs::read("test_assets/test_image2.jpg")
                .await
                .unwrap(),
            _ => unreachable!(),
        };

        // Save the current time.
        let start_time = tokio::time::Instant::now();

        // Send the image file to the central device.
        ble.send_message(image.into()).await;

        // Wait for another message to be received.
        loop {
            let message = ble.receive_message().await;
            if let BleMessage::Text(message) = message.convert_to_text().unwrap() {
                if message == "Ready" {
                    break;
                }
            }
        }

        // Save the duration taken
        time_records.push(tokio::time::Instant::now() - start_time);
    }

    // Stop the BLE peripheral engine.
    ble.stop_engine().await;
}
