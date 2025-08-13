using System;
using Cysharp.Threading.Tasks;
using PoliceThief.Game.Logic;
using UnityEngine;
using TMPro;
using UnityEngine.UI;
using PoliceThief.Core.Logging;

namespace PoliceThief.Game.Interfaces
{
    public class LoginInterface : MonoBehaviour
    {
        [SerializeField] private TMP_InputField nickNameInput;
        [SerializeField] private Button guestLoginBtn;
        [SerializeField] private Button kakaoLoginBtn;
        [SerializeField] private Button googleLoginBtn;
        [SerializeField] private Button appleLoginBtn;
        [SerializeField] private GameObject registerPanel;
        [SerializeField] private GameObject loadingPanel;
        [SerializeField] private GameObject errorPanel;
        
        private enum LoginType { Guest = 1, Kakao = 2, Google = 3, Apple = 4 }
        
        private void Start()
        {
            float loadingPanelAlpha = loadingPanel.GetComponent<CanvasGroup>().alpha;
            loadingPanel.SetActive(false);
            errorPanel.SetActive(false);
            // Unity Button은 async 메서드를 직접 연결할 수 없으므로 wrapper 사용
            guestLoginBtn.onClick.AddListener(() => LoginAction(LoginType.Guest));
            kakaoLoginBtn.onClick.AddListener(() => LoginAction(LoginType.Kakao));
            googleLoginBtn.onClick.AddListener(() => LoginAction(LoginType.Google));
            appleLoginBtn.onClick.AddListener(() => LoginAction(LoginType.Apple));
        }

        private void LoginAction(LoginType loginType)
        {
            // async 호출을 위한 wrapper
            LoginActionAsync(loginType).Forget();
        }

        private async UniTaskVoid LoginActionAsync(LoginType loginType)
        {   
            loadingPanel.SetActive(true);
            try
            {
                if (LoginService.Instance == null)
                {
                    Log.Error("LoginService instance not found.", "LoginUI");
                    return;
                }

                string nickname = string.IsNullOrEmpty(nickNameInput.text) ? "DefaultUser" : nickNameInput.text;
                
                Log.Info($"Attempting login with type: {loginType}, nickname: {nickname}", "LoginUI");
                
                bool success = await LoginService.Instance.AsyncLogin((int)loginType, nickname);
                
                if (success)
                {
                    Log.Info($"Login successful for {nickname}", "LoginUI");
                    loadingPanel.SetActive(false);
                    // TODO: 로그인 성공 시 다음 씬으로 이동 또는 UI 상태 변경
                }
                else
                {
                    Log.Error($"Login failed for {nickname}", "LoginUI");
                    // TODO: 로그인 실패 시 에러 메시지 표시
                }
            }
            catch (Exception ex)
            {   
                Log.Error($"Login action failed: {ex.Message}", "LoginUI");
            }
        }
    }
    
}