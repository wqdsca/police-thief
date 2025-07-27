using UnityEngine;
using UnityEngine.UI;
using TMPro;
using Michsky.MUIP;

public class MakeRoomPanel : MonoBehaviour
{
    [SerializeField] private CustomDropdown roomNameDropdown;
    [SerializeField] private TMP_InputField roomNameInputField;
    [SerializeField] private Button makeRoomBtn;
    [SerializeField] private Button cancelBtn;
    [SerializeField] private GameObject makeRoomPanel;
    [SerializeField] private SliderManager maxUserNumSlider;



    private void Start()
    {
        roomNameDropdown.SetupDropdown();
        roomNameDropdown.onValueChanged.AddListener(OnRoomNameDropdownValueChanged);
        roomNameDropdown.ChangeDropdownInfo(0);

        makeRoomBtn.onClick.AddListener(OnClickMakeRoom);
        cancelBtn.onClick.AddListener(OnClickCancel);

        roomNameInputField.gameObject.SetActive(false);
        roomNameInputField.interactable = false;
        maxUserNumSlider.mainSlider.onValueChanged.AddListener(OnMaxUserNumSliderValueChanged);
    }
    // 최대 인원 수 슬라이더 값 변경 시
    private void OnMaxUserNumSliderValueChanged(float value)
    {
        Debug.Log($"슬라이더 값 변경: {Mathf.RoundToInt(value)}");
    }

    // 방 이름 드롭다운 값 변경 시
    private void OnRoomNameDropdownValueChanged(int index)
    {
        string selectedRoomName = "";

        if (index == 4) // "직접 입력" 선택 시
        {
            roomNameInputField.gameObject.SetActive(true);
            roomNameInputField.interactable = true;

            roomNameDropdown.gameObject.SetActive(false); // 드롭다운 버튼 숨김

            selectedRoomName = roomNameInputField.text;
        }
        else
        {
            roomNameInputField.gameObject.SetActive(false);
            roomNameInputField.interactable = false;

            roomNameDropdown.gameObject.SetActive(true); // 드롭다운 버튼 다시 표시

            selectedRoomName = roomNameDropdown.selectedText.text;
        }

        Debug.Log($"[드롭다운] 인덱스: {index}, 선택된 방 이름: {selectedRoomName}");
    }

    private async void OnClickMakeRoom()
    {
        if(GetSelectedRoomName() == "" || GetSelectedMaxUserNum() < 2) {
            Debug.LogWarning("❗ 방 이름을 입력해주세요. 최대 인원 수는 2명 이상 설정해주세요.");
            return;
        }
        await RoomHandler.CreateRoom(GetSelectedRoomName(), GetSelectedMaxUserNum());
        OnClickCancel();
    }

    private void OnClickCancel()
    {
        makeRoomPanel.SetActive(false);
        Debug.Log("[취소] MakeRoomPanel 비활성화");
    }

    private string GetSelectedRoomName() {
        if(roomNameInputField.gameObject.activeSelf) {
            return roomNameInputField.text;
        }
        else {
            return roomNameDropdown.selectedText.text;
        }
    }
    private int GetSelectedMaxUserNum() {
        return Mathf.RoundToInt(maxUserNumSlider.mainSlider.value);
    }
}
