using UnityEngine;
using Cysharp.Threading.Tasks;
using System;
using Model.Auth;
using Model.RoomLobby;
using System.Collections.Generic;
using System.Linq;

public static class RoomHandler {
    public static async UniTask CreateRoom(string roomName, int maxUserNum) {
        try {
            // // var userId = LoginModel.loginResponseModel.id;
            // // var nickName = LoginModel.loginResponseModel.nickname;
            // if(userId == 0 || nickName == null) return;
            //테스트용 으로 만듬
            int userId = 1;
            string nickName = "test";
            int currentUserNum = 1;
            var body = new RoomLobbyRequestModel(roomName, maxUserNum, userId, nickName);
            var response = await ApiService.Instance.Request<RoomLobbyResponseModel>("POST", "/roomList/create", body, false);
            RoomLobbyService.CreateRoom(response.roomId, response.hostNickName, response.roomName, currentUserNum, response.maxUserNum);
        } catch (Exception ex) {
            Debug.LogError("방 생성 에러: " + ex.Message);
        }
    }

    public static async UniTask<bool> getRoomList(int lastRoomId) {
        try {
            // var response = await ApiService.Instance.Request<List<RoomLobbyResponseModel>>("GET", "/roomList/getList", lastRoomId, false); // 추후 api 연동을 바꿈
            var response = TestRoomList.roomList.Where(room => room.roomId > lastRoomId).ToList();
           await RoomLobbyService.GetRoomList(response);
           return true;
        } catch (Exception ex) {
            Debug.LogError("방 목록 조회 에러: " + ex.Message);
            return false;
        }
    }
}