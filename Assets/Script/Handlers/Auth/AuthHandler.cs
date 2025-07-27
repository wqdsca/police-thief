using UnityEngine;
using Cysharp.Threading.Tasks;
using System;
using Model.Auth;

public static class AuthHandler {
   
   public static async UniTask login(string loginType, string token = null) {
    try {
        token = token ?? "test";
        var body = new LoginRequestModel(loginType, token);
        var response = await ApiService.Instance.Request<LoginResponseModel>("POST", "/auth/login", body, false);
        Debug.Log($"로그인 응답: {response.status}, {response.id}, {response.nickname}, {response.isRegister}, {response.accessToken}, {response.refreshToken}");
        await LoginService.Login(response.id, response.nickname, response.isRegister, response.accessToken, response.refreshToken);

    } catch (Exception ex) {
        Debug.LogError("Login 에러: " + ex.Message);
   }
   }
}