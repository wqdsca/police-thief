using UnityEngine;
using System.Net;
using System.Net.Sockets;
using System.Threading.Tasks;
using System;

public class UdpServer : MonoBehaviour
{
    public static UdpServer Instance { get; private set; }

    private UdpClient udpClient;
    public IPEndPoint serverEndPoint;

    private int serverPort = 7000;
    private string serverIp = "127.0.0.1";

    private void Awake()
    {
        if (Instance == null)
        {
            Instance = this;
            DontDestroyOnLoad(gameObject);
        }
        else
        {
            Destroy(gameObject);
        }
    }

    private void Start()
    {
        try
        {
            udpClient = new UdpClient();
            serverEndPoint = new IPEndPoint(IPAddress.Parse(serverIp), serverPort);

            // 연결 확인용 1바이트 패킷 전송
            byte[] authPacket = new byte[] { 0x00 };
            udpClient.Send(authPacket, authPacket.Length, serverEndPoint);
            Debug.Log("0x00 연결확인 패킷 전송");

            StartReceiving();
        }
        catch (Exception e)
        {
            Debug.LogError("UDP 초기화 실패: " + e.Message);
        }
    }

    private async void StartReceiving()
    {
        while (true)
        {
            try
            {
                UdpReceiveResult result = await udpClient.ReceiveAsync();
                byte[] receivedData = result.Buffer;
               // PacketHandler.HandlerPacket(receivedData); // 수신 처리
            }
            catch (Exception e)
            {
                Debug.LogError("UDP 수신 오류: " + e.Message);
            }
        }
    }

    public void Send(byte[] data)
    {
        if (udpClient != null && serverEndPoint != null)
        {
            udpClient.Send(data, data.Length, serverEndPoint);
        }
    }
}
