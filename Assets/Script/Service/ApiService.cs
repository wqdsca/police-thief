using System;
using System.Text;
using System.Net.Http;
using System.Threading.Tasks;
using UnityEngine;
using UnityEngine.Networking;
using Cysharp.Threading.Tasks;
using Model.Auth;
using Newtonsoft.Json;

public class ApiService : MonoBehaviour
{
    public static ApiService Instance;

    [SerializeField] private string baseUrl = "http://localhost:4000/api";
    private static readonly HttpClient client = new HttpClient();

    private void Awake()
    {
        if (Instance == null)
        {
            Instance = this;
            DontDestroyOnLoad(gameObject);
        }
        else
        {
            Destroy(gameObject);
        }
    }

    // ✅ T 리턴 방식으로 리팩토링
    public async UniTask<T> Request<T>(string method, string endpoint,object body, bool isAuthRequired = true)
{
    string url = $"{baseUrl}{endpoint}";
    string jsonBody = JsonUtility.ToJson(body);
    byte[] bodyRaw = Encoding.UTF8.GetBytes(jsonBody);
    

    UnityWebRequest request = CreateRequest(method, url, bodyRaw);
    if (request == null)
        throw new Exception("❌ 지원하지 않는 HTTP 메서드입니다.");

    if (isAuthRequired)
        ApplyHeaders(request);

    await request.SendWebRequest().ToUniTask();

    return await HandleResponse<T>(request, () => Request<T>(method, endpoint, body, isAuthRequired));
}


    private UnityWebRequest CreateRequest(string method, string url, byte[] bodyRaw)
    {
        UnityWebRequest req = null;
        switch (method.ToUpper())
        {
            case "GET":
                req = UnityWebRequest.Get(url);
                break;

            case "POST":
            case "PUT":
            case "DELETE":
                req = new UnityWebRequest(url, method.ToUpper());
                req.uploadHandler = new UploadHandlerRaw(bodyRaw);
                req.downloadHandler = new DownloadHandlerBuffer();
                req.SetRequestHeader("Content-Type", "application/json");
                break;
        }
        return req;
    }

    private void ApplyHeaders(UnityWebRequest request)
    {
        string token = LoginModel.loginResponseModel.accessToken;
        if (!string.IsNullOrEmpty(token))
        {
            request.SetRequestHeader("Authorization", $"Bearer {token}");
        }
    }

    private async UniTask<T> HandleResponse<T>(UnityWebRequest request, Func<UniTask<T>> retryFunc)
{
    if (request.result != UnityWebRequest.Result.Success)
        throw new Exception("요청 실패: " + request.error);

    string jsonText = request.downloadHandler.text;
    Debug.Log($"[응답 원문] {jsonText}");

    ServerResponse<T> res;
    try
    {
        res = JsonConvert.DeserializeObject<ServerResponse<T>>(jsonText);
    }
    catch (Exception ex)
    {
        Debug.LogError($"❌ JSON 파싱 실패: {ex.Message}");
        throw new Exception("응답 파싱 실패");
    }

    switch (res.status)
    {
        case 200:
            return res.data;

        case 401:
            Debug.LogWarning("⚠️ 401 Unauthorized - RefreshToken 시도 중");
            bool refreshed = await TryRefreshToken();
            if (refreshed)
            {
                Debug.Log("🔄 토큰 갱신 성공 - 요청 재시도");
                return await retryFunc();
            }
            else
            {
                throw new Exception("인증 실패. 다시 로그인하세요.");
            }

        default:
            throw new Exception(res.error ?? "서버 오류");
    }
}


    private async Task<bool> TryRefreshToken()
    {
        var refreshToken = LoginModel.loginResponseModel.refreshToken;
        if (string.IsNullOrEmpty(refreshToken))
            return false;

        var bodyJson = JsonUtility.ToJson(new RefreshRequest { refreshToken = refreshToken });
        var content = new StringContent(bodyJson, Encoding.UTF8, "application/json");

        try
        {
            var response = await client.PostAsync($"{baseUrl}/auth/refresh", content);
            if (response.IsSuccessStatusCode)
            {
                var resultJson = await response.Content.ReadAsStringAsync();
                var tokenData = JsonUtility.FromJson<TokenResponse>(resultJson);

                LoginModel.loginResponseModel.accessToken = tokenData.accessToken;
                LoginModel.loginResponseModel.refreshToken = tokenData.refreshToken;
                return true;
            }
        }
        catch (Exception ex)
        {
            Debug.LogError("RefreshToken 에러: " + ex.Message);
        }

        return false;
    }

    [Serializable]
    public class ServerResponse<T>
    {
        public int status;
        public T data;
        public string error;
    }

    [Serializable]
    private class RefreshRequest
    {
        public string refreshToken;
    }

    [Serializable]
    private class TokenResponse
    {
        public string accessToken;
        public string refreshToken;
    }
}
