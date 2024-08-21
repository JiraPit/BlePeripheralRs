mod bluetooth;

use bluetooth::message::BleMessage;
use bluetooth::BlePeripheral;
use std::io::Cursor;
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
        let img = match i % 2 {
            0 => image::open("test_assets/test_image1.jpg").unwrap(),
            1 => image::open("test_assets/test_image2.jpg").unwrap(),
            _ => unreachable!(),
        };

        // Save the current time.
        let start_time = tokio::time::Instant::now();

        // Resize the image
        let img = img.resize_exact(75, 100, image::imageops::FilterType::Nearest);

        // Convert the image to a byte array.
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
            .unwrap();

        let duration = tokio::time::Instant::now() - start_time;
        println!("Image preprocessed {}: {:?}", i, duration);

        // Send the image file size to the central device.
        ble.send_message(bytes.len().into()).await;

        // Send the image file to the central device.
        ble.send_message(bytes.into()).await;

        let duration = tokio::time::Instant::now() - start_time;
        println!("Image sent {}: {:?}", i, duration);

        // Wait for a confirmation to be received.
        loop {
            let message = ble.receive_message().await;
            if let BleMessage::Text(message) = message.convert_to_text().unwrap() {
                if message == "Ready" {
                    break;
                }
            }
        }

        // Save the duration taken
        let duration = tokio::time::Instant::now() - start_time;
        time_records.push(duration);
        println!("Confirmation received for image {}: {:?}", i, duration);
    }

    // Calculate the average duration.
    let sum: tokio::time::Duration = time_records.iter().sum();
    let average = sum / time_records.len() as u32;
    println!("Average total delay: {:?}", average);

    // Stop the BLE peripheral engine.
    ble.stop_engine().await;
}
