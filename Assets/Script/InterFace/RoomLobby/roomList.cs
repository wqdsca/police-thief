using UnityEngine;
using UnityEngine.UI;
using Sirenix.OdinInspector;
using Cysharp.Threading.Tasks;
using System.Collections.Generic;
using System;
using Model.RoomLobby;
using TMPro;
using System.Linq;
public class roomList : MonoBehaviour
{
    [SerializeField, BoxGroup("RoomItem"), InfoBox("룸 아이템 프리팹")] 
    private GameObject roomItem;
    [SerializeField, BoxGroup("RoomItem"), InfoBox("룸 이름 텍스트")] 
    private TextMeshProUGUI roomNameText;
    [SerializeField, BoxGroup("RoomItem"), InfoBox("룸 플레이어 수 텍스트")] 
    private TextMeshProUGUI currentNumAndMaxNumText;
    [SerializeField, BoxGroup("RoomItem"), InfoBox("스크롤 뷰 스크롤")] 
    private ScrollRect scrollRect;

    [SerializeField, BoxGroup("RoomItem"), InfoBox("룸 리스트 컨텐츠 담는 공간")] 
    private Transform roomListContent;
    public static roomList instance { get; private set; }

    public int lastRoomId;

    private void Awake()
    {
        if(instance == null)
        {
            instance = this;
        }
        else
        {
            Destroy(gameObject);
        }
    }

    public async UniTask getRoomList()
    {
        int count = 0;
        bool success = false;

        while (!success)
        {   
            success = await RoomHandler.getRoomList(lastRoomId);
            Debug.Log("isLoading: " + success);
            if (success) break;

            await UniTask.Delay(1000);
            count++;
            Debug.Log($"방 목록 조회 실패 {count}번 실행");

            if (count > 5)
            {
                Debug.Log("방 목록 조회 실패 5회 초과 실행으로 실패");
                return;
            }
        }

       InstantiateRoomItem();
    }

    // 룸 데이터 복사하기
    private void InstantiateRoomItem() {
         foreach (var room in RoomList.GetAllRooms())
{   var item = Instantiate(roomItem, roomListContent);
    var data = room.Value;
    roomNameText.text = data.roomName;
    currentNumAndMaxNumText.text = $"{data.currentUserNum}/{data.maxUserNum}";
    }
    roomList.instance.lastRoomId = RoomList.GetAllRooms().Keys.Max();
    Debug.Log("lastRoomId: " + roomList.instance.lastRoomId);
    }   
    // 룸 리스트 스크롤 시 데이터 더 요청하기
    // private void OnScrollRectValueChanged(Vector2 value)
    // {   
    //     if(scrollRect.verticalNormalizedPosition < 0.1f) {
    //         var response = await RoomHandler.getRoomList(roomList.instance.lastRoomId);
    //         if(response.count >0) {

    //         }
    //     }
    //     Debug.Log("스크롤 뷰 스크롤 값: " + value);
    // }

}
