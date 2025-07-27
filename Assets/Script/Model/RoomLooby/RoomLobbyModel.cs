namespace Model.RoomLobby
{
using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using Sirenix.OdinInspector;
using Cysharp.Threading.Tasks;
using Michsky.MUIP;
using System.Linq;

public static class TestRoomList {
    public static List<RoomLobbyResponseModel> roomList = new List<RoomLobbyResponseModel> {
        new RoomLobbyResponseModel(1, "test1", "test1", 1, 5),
        new RoomLobbyResponseModel(2, "test2", "test2", 1, 8),
        new RoomLobbyResponseModel(3, "test3", "test3", 1, 7),
        new RoomLobbyResponseModel(4, "test4", "test4", 1, 6),
        new RoomLobbyResponseModel(5, "test5", "test5", 1, 5),
        new RoomLobbyResponseModel(6, "test6", "test6", 1, 4),
        new RoomLobbyResponseModel(7, "test7", "test7", 1, 3),
        new RoomLobbyResponseModel(8, "test8", "test8", 1, 2),
        new RoomLobbyResponseModel(9, "test9", "test9", 1, 1),
        new RoomLobbyResponseModel(10, "test10", "test10", 1, 0),
        new RoomLobbyResponseModel(11, "test11", "test11", 1, 0),
        new RoomLobbyResponseModel(12, "test12", "test12", 1, 0),
        new RoomLobbyResponseModel(13, "test13", "test13", 1, 0),
        new RoomLobbyResponseModel(14, "test14", "test14", 1, 0),
        new RoomLobbyResponseModel(15, "test15", "test15", 1, 0),
        new RoomLobbyResponseModel(16, "test16", "test16", 1, 0),
        new RoomLobbyResponseModel(17, "test17", "test17", 1, 0),
        new RoomLobbyResponseModel(18, "test18", "test18", 1, 0),
        new RoomLobbyResponseModel(19, "test19", "test19", 1, 0),
        new RoomLobbyResponseModel(20, "test20", "test20", 1, 0),
        new RoomLobbyResponseModel(21, "test21", "test21", 1, 0),
        new RoomLobbyResponseModel(22, "test22", "test22", 1, 0),
        new RoomLobbyResponseModel(23, "test23", "test23", 1, 0),
        new RoomLobbyResponseModel(24, "test24", "test24", 1, 0),
        new RoomLobbyResponseModel(25, "test25", "test25", 1, 0),
        new RoomLobbyResponseModel(26, "test26", "test26", 1, 0),
        new RoomLobbyResponseModel(27, "test27", "test27", 1, 0),
        new RoomLobbyResponseModel(28, "test28", "test28", 1, 0),
        new RoomLobbyResponseModel(29, "test29", "test29", 1, 0),
        new RoomLobbyResponseModel(30, "test30", "test30", 1, 0),
        new RoomLobbyResponseModel(31, "test31", "test31", 1, 0),
        new RoomLobbyResponseModel(32, "test32", "test32", 1, 0),
        new RoomLobbyResponseModel(33, "test33", "test33", 1, 0),
        new RoomLobbyResponseModel(34, "test34", "test34", 1, 0),
        new RoomLobbyResponseModel(35, "test35", "test35", 1, 0),
        new RoomLobbyResponseModel(36, "test36", "test36", 1, 0),
        new RoomLobbyResponseModel(37, "test37", "test37", 1, 0),
        new RoomLobbyResponseModel(38, "test38", "test38", 1, 0),
        new RoomLobbyResponseModel(39, "test39", "test39", 1, 0),
        new RoomLobbyResponseModel(40, "test40", "test40", 1, 0),
        new RoomLobbyResponseModel(41, "test41", "test41", 1, 0),
        new RoomLobbyResponseModel(42, "test42", "test42", 1, 0),
    };
}


[Title("RoomLobbyModel")]
[InlineEditor]
public class RoomLobbyRequestModel
{
    public string roomName;
    public int maxUserNum;
    public int userId;
    public string nickName;
    public RoomLobbyRequestModel(string roomName, int maxUserNum, int userId, string nickName) {
        this.roomName = roomName;
        this.maxUserNum = maxUserNum;
        this.userId = userId;
        this.nickName = nickName;
    }
}

[Title("RoomLobbyResponseModel")]
[InlineEditor]
public class RoomLobbyResponseModel
{
    public int roomId;
    public string hostNickName;
    public string roomName;
    public int currentUserNum;
    public int maxUserNum;
    public RoomLobbyResponseModel(int roomId, string hostNickName, string roomName, int currentUserNum, int maxUserNum) {
        this.roomId = roomId;
        this.hostNickName = hostNickName;
        this.roomName = roomName;
        this.currentUserNum = currentUserNum;
        this.maxUserNum = maxUserNum;
    }
}
public static class RoomList {
    private static Dictionary<int, RoomLobbyResponseModel> rooms = new();

    public static void AddRoom(RoomLobbyResponseModel room) {
        if (!rooms.ContainsKey(room.roomId)) {
            rooms.Add(room.roomId, room);
        } else {
            Debug.LogWarning($"Room {room.roomId} already exists.");
        }
    }

    public static void RemoveRoom(int roomId) {
        rooms.Remove(roomId);
    }

    public static void UpdateRoom(RoomLobbyResponseModel room) {
        rooms[room.roomId] = room;
    }

    public static void ClearRoom() {
        rooms.Clear();
    }

    public static RoomLobbyResponseModel GetRoom(int roomId) {
        if (rooms.TryGetValue(roomId, out var room)) return room;
        Debug.LogWarning($"Room {roomId} not found.");
        return null;
    }

    public static Dictionary<int, RoomLobbyResponseModel> GetAllRooms() {
            return rooms;
        }
    }
}
