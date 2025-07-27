using Cysharp.Threading.Tasks;
using UnityEngine;
using Model.RoomLobby;
using Model.Auth;
using Sirenix.OdinInspector;
using System;
using System.Collections.Generic;

public static class RoomLobbyService {
    public static void CreateRoom(int roomId, string hostNickName, string roomName, int currentUserNum, int maxUserNum) {
        try {
            if(roomId != 0 && hostNickName != null && roomName != null && maxUserNum != 0) {
                Debug.Log("✅ 방 생성 성공");
                 RoomList.AddRoom(new RoomLobbyResponseModel(roomId, hostNickName, roomName, currentUserNum, maxUserNum));
                
            }
        } catch (Exception ex) {
            Debug.LogError("❌ 방 생성 에러: " + ex.Message);
          
        }
    }
    public static void GetRoomList(List<RoomLobbyResponseModel> roomList) {
        try {
            if(roomList.Count == 0) {
                Debug.Log("❌ 방 목록 조회 에러: 방 목록이 없습니다.");
                return;
            }
            if(roomList != null) {
                foreach(var room in roomList) {
                    RoomList.AddRoom(room);
                }
            }
        } catch (Exception ex) {
            Debug.LogError("❌ 방 목록 조회 에러: " + ex.Message);
        }
    }
}