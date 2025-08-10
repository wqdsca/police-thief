use rudpserver::connection::ConnectionManager;
use rudpserver::server::RudpServer;
use rudpserver::session::SessionManager;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

#[cfg(test)]
mod server_integration_tests {
    use super::*;

    async fn create_test_server() -> (RudpServer, SocketAddr) {
        let server = RudpServer::new("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        (server, addr)
    }

    async fn create_test_client() -> UdpSocket {
        UdpSocket::bind("127.0.0.1:0").unwrap()
    }

    #[tokio::test]
    async fn test_server_startup_shutdown() {
        let (server, addr) = create_test_server().await;
        assert!(addr.port() > 0);

        let handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.abort();
    }

    #[tokio::test]
    async fn test_client_connection() {
        let (server, server_addr) = create_test_server().await;
        let server_handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = create_test_client().await;
        let syn_packet = create_syn_packet();
        client.send_to(&syn_packet, server_addr).unwrap();

        let mut buffer = [0u8; 1500];
        let result = timeout(
            Duration::from_secs(1),
            tokio::task::spawn_blocking(move || client.recv_from(&mut buffer)),
        )
        .await;

        assert!(result.is_ok());
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_multiple_connections() {
        let (server, server_addr) = create_test_server().await;
        let server_handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut clients = vec![];
        for _ in 0..10 {
            let client = create_test_client().await;
            let syn_packet = create_syn_packet();
            client.send_to(&syn_packet, server_addr).unwrap();
            clients.push(client);
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        for client in clients {
            let mut buffer = [0u8; 1500];
            let _ = client.set_read_timeout(Some(Duration::from_millis(100)));
            let _ = client.recv_from(&mut buffer);
        }

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_data_transmission() {
        let (server, server_addr) = create_test_server().await;
        let server_handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = create_test_client().await;

        let syn_packet = create_syn_packet();
        client.send_to(&syn_packet, server_addr).unwrap();

        let mut buffer = [0u8; 1500];
        let _ = client.recv_from(&mut buffer);

        let test_data = b"Hello, RUDP Server!";
        let data_packet = create_data_packet(1, test_data.to_vec());
        client.send_to(&data_packet, server_addr).unwrap();

        let (size, _) = client.recv_from(&mut buffer).unwrap();
        assert!(size > 0);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let conn_manager = Arc::new(RwLock::new(ConnectionManager::new(Duration::from_millis(
            100,
        ))));

        let test_addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        {
            let mut manager = conn_manager.write().await;
            manager.create_connection(test_addr);
        }

        tokio::time::sleep(Duration::from_millis(200)).await;

        {
            let manager = conn_manager.read().await;
            assert!(!manager.is_connected(&test_addr));
        }
    }

    #[tokio::test]
    async fn test_session_management() {
        let session_manager = Arc::new(RwLock::new(SessionManager::new()));
        let test_addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let session_id = {
            let mut manager = session_manager.write().await;
            manager.create_session(test_addr)
        };

        assert!(session_id > 0);

        {
            let manager = session_manager.read().await;
            let session = manager.get_session(session_id);
            assert!(session.is_some());
            assert_eq!(session.unwrap().addr, test_addr);
        }

        {
            let mut manager = session_manager.write().await;
            manager.remove_session(session_id);
        }

        {
            let manager = session_manager.read().await;
            assert!(manager.get_session(session_id).is_none());
        }
    }

    #[tokio::test]
    async fn test_packet_ordering() {
        let (server, server_addr) = create_test_server().await;
        let server_handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = create_test_client().await;

        let syn_packet = create_syn_packet();
        client.send_to(&syn_packet, server_addr).unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        for i in (0..10).rev() {
            let data_packet = create_data_packet(i, vec![i as u8]);
            client.send_to(&data_packet, server_addr).unwrap();
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let conn_manager = Arc::new(RwLock::new(ConnectionManager::new(Duration::from_secs(30))));

        let mut handles = vec![];

        for i in 0..50 {
            let manager = conn_manager.clone();
            let handle = tokio::spawn(async move {
                let addr: SocketAddr = format!("127.0.0.1:{}", 10000 + i).parse().unwrap();

                {
                    let mut m = manager.write().await;
                    m.create_connection(addr);
                }

                tokio::time::sleep(Duration::from_millis(10)).await;

                {
                    let m = manager.read().await;
                    m.is_connected(&addr)
                }
            });
            handles.push(handle);
        }

        let results = futures::future::join_all(handles).await;
        for result in results {
            assert!(result.unwrap());
        }
    }

    fn create_syn_packet() -> Vec<u8> {
        let mut packet = vec![0x01];
        packet.extend_from_slice(&0u32.to_le_bytes());
        packet.extend_from_slice(&0u64.to_le_bytes());
        packet
    }

    fn create_data_packet(seq: u32, data: Vec<u8>) -> Vec<u8> {
        let mut packet = vec![0x03];
        packet.extend_from_slice(&seq.to_le_bytes());
        packet.extend_from_slice(&0u64.to_le_bytes());
        packet.extend_from_slice(&(data.len() as u32).to_le_bytes());
        packet.extend_from_slice(&data);
        packet
    }
}
