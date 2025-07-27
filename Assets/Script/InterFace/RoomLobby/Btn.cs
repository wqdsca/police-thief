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
    [SerializeField] private ListView roomListPanel;

    private async void Awake()
    {   MakeRoomPanel.SetActive(false);
        makeRoomBtn.onClick.AddListener(MakeRoom);
        earlyJoinBtn.onClick.AddListener(EarlyJoin);
        getRoomListBtn.onClick.AddListener(GetRoomList);
        exitBtn.onClick.AddListener(Exit);
        await UniTask.Delay(1000);
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
{
    Debug.Log("GetRoomList");

    // roomListPanel.listItems.Clear(); // 🔄 기존 리스트 초기화
    // await getRoomListTest();         // 🧪 테스트 데이터 추가
    // await roomListPanel.InitializeItems(); // ✅ UI 렌더링

    // if (RoomList.GetAllRooms().Count == 0)
    // {
    //     Debug.Log("❌ 방 목록 조회 에러: 방 목록이 없습니다.");
    //     return;
    // }
}

    private void Exit()
    {
        Debug.Log("Exit");
    }
    // 테스트용 코드 
//    private async UniTask getRoomListTest()
// {
//     await UniTask.Delay(1000);
    
//     foreach (var room in TestRoomList.roomList)
//     {
//         var item = new ListView.ListItem(); // ← 여기서 매번 새로 생성해야 함
//         item.row(room.roomId.ToString());
//         item.row(room.hostNickName);
//         item.row(room.roomName);
//         item.row(room.currentUserNum.ToString());
//         item.row(room.maxUserNum.ToString());

//         roomListPanel.listItems.Add(item);
//         TestRoomList.AddRoom(room);
//     }
// }

}



