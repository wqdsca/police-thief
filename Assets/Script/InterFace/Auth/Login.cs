using UnityEngine;
using UnityEngine.UI;
using Cysharp.Threading.Tasks;
using TMPro;

public class Login : MonoBehaviour
{
    [SerializeField] private Button loginBtn;
    [SerializeField] private TMP_InputField idInputField;


    private async void Awake() {
        loginBtn.onClick.AddListener(loginAction);
        await UniTask.Delay(1000);
    }
    public async void loginAction() {
        if (ApiService.Instance == null) return;
        await AuthHandler.login(idInputField.text.Trim());
    }
}