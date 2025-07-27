namespace Model.Auth
{
    using Cysharp.Threading.Tasks;
    [System.Serializable]
    public class LoginResponseModel
    {
        public int status;
        public int id;
        public string nickname;
        public bool isRegister;
        public string accessToken;
        public string refreshToken;

        public LoginResponseModel(int id, string nickname, string accessToken, string refreshToken)
        {
            this.id = id;
            this.nickname = nickname;
            this.accessToken = accessToken;
            this.refreshToken = refreshToken;
        }
    }

    public static class LoginModel
    {
        public static LoginResponseModel loginResponseModel;

        public static async UniTask Set(int id, string nickname, string accessToken, string refreshToken)
        {
            loginResponseModel = new LoginResponseModel(id, nickname, accessToken, refreshToken);
            await UniTask.CompletedTask;
        }
    }
}

[System.Serializable]
public class LoginRequestModel
{
    public string loginType;
    public string token;
    public LoginRequestModel(string loginType, string token)
    {
        this.loginType = loginType;
        this.token = token;
    }
}

