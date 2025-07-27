using Cysharp.Threading.Tasks;
using UnityEngine;
using System;
using UnityEngine.SceneManagement;
using Model.Auth;
using Sirenix.OdinInspector;

[Button("Login")]
public static class LoginService
{
    public static async UniTask<bool> Login(int id, string nickname, bool isRegister, string accessToken, string refreshToken)
    {
        try
        {
            if (id != 0 && nickname != null && accessToken != null && refreshToken != null)
            {
                if(!isRegister) {
                    Debug.Log("✅ 회원가입 화면으로 이동해야함");
                }
                else {
                    Debug.Log("✅ 로그인 성공");
                   await LoginModel.Set(id, nickname, accessToken, refreshToken);
                   await SceneManager.LoadSceneAsync("roomList");
                }
                return true;
            }
            else
            {
                Debug.LogError("❌ Login 실패");
                return false;
            }
        }
        catch (Exception ex)
        {
            Debug.LogError("❌ Login 에러: " + ex.Message);
            return false;
        }
    }
}
