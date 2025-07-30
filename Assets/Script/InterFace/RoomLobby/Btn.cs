using UnityEngine;
using UnityEngine.UI;
using Cysharp.Threading.Tasks;
using Sirenix.OdinInspector;
using Michsky.MUIP;
using Model.RoomLobby;


public class Btn : MonoBehaviour
{
    [SerializeField] [Title("MakeRoom")] [InfoBox("방만들기")] private Button makeRoomBtn;
    [SerializeField] private Button earlyJoinBtn;
    [SerializeField] private Button getRoomListBtn;
    [SerializeField] private Button exitBtn;
    [SerializeField] private GameObject MakeRoomPanel;
    [SerializeField] private GameObject  roomListPanel;

    private void Awake()
    {   MakeRoomPanel.SetActive(false);
        roomListPanel.SetActive(false);
        makeRoomBtn.onClick.AddListener(MakeRoom);
        earlyJoinBtn.onClick.AddListener(EarlyJoin);
        getRoomListBtn.onClick.AddListener(GetRoomList);
        exitBtn.onClick.AddListener(Exit);
    }
    [Button("MakeRoom")]
    private void MakeRoom()
    {   Debug.Log("MakeRoom");
        MakeRoomPanel.SetActive(true);
    }
    private void EarlyJoin()
    {
        Debug.Log("EarlyJoin");
    }
    private async void GetRoomList()
{   roomList.instance.lastRoomId = 0;
    await RoomList.ClearRoom();
    Debug.Log("lastRoomId 업데이트 " + roomList.instance.lastRoomId);
    Debug.Log("GetRoomList");
    roomListPanel.SetActive(true);
    await roomList.instance.getRoomList();
    // roomListPanel.listItems.Clear(); // 🔄 기존 리스트 초기화
    // await getRoomListTest();         // 🧪 테스트 데이터 추가
    // await roomListPanel.InitializeItems(); // ✅ UI 렌더링

    // if (RoomList.GetAllRooms().Count == 0)
    // {
    //     Debug.Log("❌ 방 목록 조회 에러: 방 목록이 없습니다.");
    //     return;
    // }
}   
private async void renewRoomList() {

}

    private void Exit()
    {
        Debug.Log("Exit");
    }
    
}



