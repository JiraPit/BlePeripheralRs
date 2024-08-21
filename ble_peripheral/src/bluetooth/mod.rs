pub mod message;
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
use std::error::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, watch},
    task::JoinHandle,
};
use uuid::Uuid;

static SERVICE_UUID: Uuid = Uuid::from_u128(0x0000181C00001000800000805F9B34FB);
static CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x00002AC400001000800000805F9B34FB);

/// BLE peripheral utility.
/// For creating a BLE peripheral device that can be connected to a central device.
pub struct BlePeripheral {
    pub alias: Option<String>,
    sender: Option<mpsc::UnboundedSender<BleMessage>>,
    receiver: Option<mpsc::UnboundedReceiver<BleMessage>>,
    app_handler: Option<ApplicationHandle>,
    adv_handler: Option<AdvertisementHandle>,
    ble_thread: Option<JoinHandle<()>>,
    subscribed_watcher: Option<watch::Receiver<bool>>,
}

impl BlePeripheral {
    /// Create a new BLE peripheral with the given alias.
    pub async fn new(alias: Option<String>) -> Result<BlePeripheral, Box<dyn Error>> {
        let sender = None;
        let reader = None;
        let app_handler = None;
        let adv_handler = None;
        let ble_thread = None;
        let subscribed_watcher = None;

        Ok(BlePeripheral {
            sender,
            receiver: reader,
            alias,
            app_handler,
            adv_handler,
            ble_thread,
            subscribed_watcher,
        })
    }

    /// Start the BLE peripheral advertising and GATT service
    pub async fn start_engine(&mut self) -> Result<(), Box<dyn Error>> {
        // Initialize the BLE session and adapter
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;
        adapter.set_discoverable(true).await.unwrap();
        adapter.set_discoverable_timeout(0).await.unwrap();

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
                        write: true,
                        write_without_response: false,
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

        // Initialize the send channel
        let (send_tx, mut send_rx) = mpsc::unbounded_channel();
        self.sender = Some(send_tx);

        // Initialize the receive channel
        let (receive_tx, receive_rx) = mpsc::unbounded_channel();
        self.receiver = Some(receive_rx);

        // Initialize the subscribed watcher
        let (subscribed_watch_tx, subscribed_watch_rx) = watch::channel(false);
        self.subscribed_watcher = Some(subscribed_watch_rx);

        // Start the BLE thread
        let ble_thread = tokio::spawn(async move {
            pin_mut!(char_control);

            // Initialize the read buffer and notifier/reciever operators
            let mut receive_buf = Vec::new();
            let mut receiver_opt: Option<CharacteristicReader> = None;
            let mut notifier_opt: Option<CharacteristicWriter> = None;

            loop {
                // Handle GATT, notify, and receive events concurrently
                tokio::select! {
                    // Handle the GATT events
                    evt = char_control.next() => {
                        match evt {
                            // Handle the write event
                            Some(CharacteristicControlEvent::Write(req)) => {
                                log::debug!("Accepting write request event with MTU {}", req.mtu());
                                receive_buf = vec![0;req.mtu()];
                                receiver_opt = Some(req.accept().unwrap());
                            },
                            // Handle the notify event
                            Some(CharacteristicControlEvent::Notify(notifier)) => {
                                log::debug!("Accepting notify request event with MTU {}", notifier.mtu());
                                notifier_opt = Some(notifier);
                                subscribed_watch_tx.send(true).unwrap();
                            },
                            _ => {},
                        }
                    },

                    // Handle the notification event
                    notify_message = send_rx.recv() => {
                        if notifier_opt.is_some() && notify_message.is_some() {
                            // Convert the message to a byte array
                            log::debug!("Notifying message {:x?}", notify_message);
                            let message_bytes = notify_message.unwrap().take_bytes();

                            // Write the message to the notify opterator
                            if let Err(err) = notifier_opt.as_mut().unwrap().write_all(&message_bytes).await {
                                log::error!("Write failed: {}", &err);
                                notifier_opt = None;
                                subscribed_watch_tx.send(false).unwrap();
                            }
                        }
                    },

                    // Handle the receive event
                    received_buffer = async {
                        match &mut receiver_opt {
                            Some(receiver) => receiver.read(&mut receive_buf).await,
                            None => future::pending().await,
                        }
                    } => {
                        match received_buffer {
                            // Message received
                            Ok(n) => {
                                // Read the message
                                let received_message = receive_buf[..n].to_vec();
                                log::debug!("Received message: {:?}", received_message);

                                // Send the message to the receiver
                                if let Err(err) = receive_tx.send(received_message.into()) {
                                    log::error!("Receive message error: {:?}", &err);
                                }
                            }

                            Err(err) => {
                                log::error!("Read stream error: {}", &err);
                            }
                        }
                        receiver_opt = None;
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
    pub async fn send_message<M>(&self, message: M) -> Result<(), Box<dyn Error>>
    where
        M: Into<BleMessage>,
    {
        let sender = match self.sender.as_ref() {
            Some(sender) => sender,
            None => {
                return Err("Send channel not initialized".into());
            }
        };
        sender.send(message.into())?;
        Ok(())
    }

    /// Receive a message from the central device.
    /// Receiving is blocking and will wait for the message if it is not ready.
    /// If there are multiple messages, the oldest one will be returned first.
    pub async fn receive_message(&mut self) -> BleMessage {
        loop {
            let message = self.receiver.as_mut().unwrap().recv().await;
            // Check if the message received is not empty, otherwise continue the loop
            if let Some(message) = message {
                return message;
            }
        }
    }

    /// Check if the BLE peripheral is subscribed to notifications.
    pub async fn is_subscribed(&self) -> bool {
        let subscribed_watcher = match self.subscribed_watcher.as_ref() {
            Some(watcher) => watcher,
            None => return false,
        };
        *subscribed_watcher.borrow()
    }
}
