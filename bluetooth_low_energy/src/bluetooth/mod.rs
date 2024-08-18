mod message;
mod test;

use bluer::{
    adv::{Advertisement, AdvertisementHandle, Type as AdvertisementType},
    gatt::{
        local::{
            characteristic_control, service_control, Application, ApplicationHandle,
            Characteristic, CharacteristicControlEvent, CharacteristicNotify,
            CharacteristicNotifyMethod, CharacteristicWrite, CharacteristicWriteMethod, Service,
        },
        CharacteristicReader, CharacteristicWriter,
    },
    Session,
};
use futures::{future, pin_mut, StreamExt};
use message::BleMessage;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Arc;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Notify, RwLock},
    task::JoinHandle,
};
use uuid::Uuid;

static SERVICE_UUID: Uuid = Uuid::from_u128(0x0000181C00001000800000805F9B34FB);
static CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x00002AC400001000800000805F9B34FB);

/// BLE peripheral utility.
/// For creating a BLE peripheral device that can be connected to a central device.
pub struct BlePeripheral {
    pub alias: Option<String>,
    send_queue: Arc<RwLock<VecDeque<BleMessage>>>,
    receive_queue: Arc<RwLock<VecDeque<BleMessage>>>,
    receive_notify: Arc<Notify>,
    app_handler: Option<ApplicationHandle>,
    adv_handler: Option<AdvertisementHandle>,
    ble_thread: Option<JoinHandle<()>>,
    subscribed: Arc<RwLock<bool>>,
}

impl BlePeripheral {
    /// Create a new BLE peripheral with the given alias.
    pub async fn new(alias: Option<String>) -> Result<BlePeripheral, Box<dyn Error>> {
        let send_queue = Arc::new(RwLock::new(VecDeque::new()));
        let read_queue = Arc::new(RwLock::new(VecDeque::new()));
        let read_notify = Arc::new(Notify::new());
        let app_handler = None;
        let adv_handler = None;
        let ble_thread = None;
        let subscribed = Arc::new(RwLock::new(false));

        Ok(BlePeripheral {
            alias,
            send_queue,
            receive_queue: read_queue,
            receive_notify: read_notify,
            app_handler,
            adv_handler,
            ble_thread,
            subscribed,
        })
    }

    /// Start the BLE peripheral advertising and GATT service
    pub async fn start_engine(&mut self) -> Result<(), Box<dyn Error>> {
        // Initialize the BLE session and adapter
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        // Configure the advertisement
        let adv = Advertisement {
            service_uuids: vec![SERVICE_UUID].into_iter().collect(),
            advertisement_type: AdvertisementType::Peripheral,
            discoverable: Some(true),
            local_name: self.alias.clone(),
            ..Default::default()
        };

        // Initialize the GATT service and characteristic handles
        let (_, service_handle) = service_control();
        let (char_control, char_handle) = characteristic_control();

        // Configure the GATT application
        let app = Application {
            services: vec![Service {
                uuid: SERVICE_UUID,
                primary: true,
                characteristics: vec![Characteristic {
                    uuid: CHARACTERISTIC_UUID,
                    write: Some(CharacteristicWrite {
                        write_without_response: true,
                        method: CharacteristicWriteMethod::Io,
                        ..Default::default()
                    }),
                    notify: Some(CharacteristicNotify {
                        notify: true,
                        method: CharacteristicNotifyMethod::Io,
                        ..Default::default()
                    }),
                    control_handle: char_handle,
                    ..Default::default()
                }],
                control_handle: service_handle,
                ..Default::default()
            }],
            ..Default::default()
        };

        // Start the BLE advertisement and GATT application
        self.adv_handler = Some(adapter.advertise(adv).await?);
        self.app_handler = Some(adapter.serve_gatt_application(app).await?);

        // Make sure that the sucscribed flaf starts as false
        {
            let mut subscribed_writer = self.subscribed.write().await;
            *subscribed_writer = false;
        }

        // Initialize the read buffer and notifier/reciever handles
        let mut receive_buf = Vec::new();
        let mut receiver_opt: Option<CharacteristicReader> = None;
        let mut notifier_opt: Option<CharacteristicWriter> = None;
        let mut notify_interval = tokio::time::interval(tokio::time::Duration::from_millis(50));

        // Clone the read queue and notify handle
        let receive_queue_clone = Arc::clone(&self.receive_queue);
        let receive_notify = Arc::clone(&self.receive_notify);
        let send_queue_clone = Arc::clone(&self.send_queue);
        let subscribed_clone = Arc::clone(&self.subscribed);

        // Start the BLE thread
        let ble_thread = tokio::spawn(async move {
            pin_mut!(char_control);
            loop {
                // Initialize the received message as an empty raw message
                let mut received_message = BleMessage::Raw(Vec::new());

                // Handle GATT, notify, and receive events concurrently
                tokio::select! {
                    // Handle the GATT events
                    evt = char_control.next() => {
                        match evt {
                            // Handle the write event
                            Some(CharacteristicControlEvent::Write(req)) => {
                                log::debug!("Accepting write request event with MTU {}", req.mtu());
                                receive_buf = Vec::new();
                                receiver_opt = Some(req.accept().unwrap());
                            },
                            // Handle the notify event
                            Some(CharacteristicControlEvent::Notify(notifier)) => {
                                log::debug!("Accepting notify request event with MTU {}", notifier.mtu());
                                notifier_opt = Some(notifier);
                                let mut subscribed_writer = subscribed_clone.write().await;
                                *subscribed_writer = true;
                            },
                            None => break,
                        }
                    },

                    // Handle the notification interval event
                    _notify_handle = notify_interval.tick() => {
                        if notifier_opt.is_some() {
                            let message: Option<BleMessage>;
                            {
                                let mut send_queue_writer =
                                    send_queue_clone.write().await;
                                message = send_queue_writer.pop_front();
                            }

                            if message.is_some() {
                                // Convert the message to a byte array
                                log::debug!("Notifying message {:x?}", message);
                                let message_bytes = message.unwrap().take_bytes();

                                // Write the message to the notify opterator
                                if let Err(err) = notifier_opt.as_mut().unwrap().write_all(&message_bytes).await {
                                    log::error!("Write failed: {}", &err);
                                    notifier_opt = None;
                                    let mut subscribed_writer = subscribed_clone.write().await;
                                    *subscribed_writer = false;
                                }
                            }
                        }
                    },

                    // Handle the receive event
                    receive_handle = async {
                        match &mut receiver_opt {
                            Some(receiver) => receiver.read_to_end(&mut receive_buf).await,
                            None => future::pending().await,
                        }
                    } => {
                        match receive_handle {
                            // Message ended
                            Ok(0) => {
                                receiver_opt = None;
                            }

                            // Message received
                            Ok(n) => {
                                // Read the message
                                let bytes = receive_buf[..n].to_vec();
                                log::debug!("Received message: {:?}", bytes);

                                // Extend the received message with the new value
                                received_message.extend_raw_bytes(bytes).unwrap();

                                // Push the message to the receive queue
                                {
                                    let mut read_queue_writer = receive_queue_clone.write().await;
                                    read_queue_writer.push_back(received_message);
                                }

                                // Notify the receiver that a message has been received
                                receive_notify.notify_one();
                            }

                            Err(err) => {
                                log::error!("Read stream error: {}", &err);
                                receiver_opt = None;
                            }
                        }
                    }
                }
            }
        });

        // Store the BLE thread handle
        self.ble_thread = Some(ble_thread);

        Ok(())
    }

    /// Stop the BLE peripheral advertising and GATT service.
    pub async fn stop_engine(&mut self) {
        if let Some(ble_thread) = self.ble_thread.take() {
            ble_thread.abort();
            ble_thread.await.unwrap_or(());
        }
        drop(self.app_handler.take());
        drop(self.adv_handler.take());
    }

    /// Send a message to the central device.
    /// This does not send the message immediately, but queues it for sending on the read event.
    /// Messages are sent in the order they are queued.
    pub async fn send_message(&self, message: BleMessage) {
        let mut send_queue = self.send_queue.write().await;
        send_queue.push_back(message);
    }

    /// Receive a message from the central device.
    /// Receiving is blocking and will wait for the message if it is not ready.
    /// If there are multiple messages, the oldest one will be returned first.
    pub async fn receive_message(&self) -> BleMessage {
        let mut message;
        loop {
            tokio::select! {

                // Try reading the message if no message notification is received
                _ = self.receive_notify.notified()=> {
                    let mut read_queue_writer = self.receive_queue.write().await;
                    message = read_queue_writer.pop_front();
                },

                // Also try reading the message after a certain delay
                // This ensures that no message is left unread
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    let mut read_queue_writer = self.receive_queue.write().await;
                    message = read_queue_writer.pop_front();
                },
            }

            // Check if the message received is not empty, otherwise continue the loop
            if let Some(message) = message {
                return message;
            }
        }
    }

    /// Check if the BLE peripheral is subscribed to notifications.
    pub async fn is_subscribed(&self) -> bool {
        let subscribed_reader = self.subscribed.read().await;
        *subscribed_reader
    }
}
