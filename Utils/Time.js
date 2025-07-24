function getCurrentTime() {
    const now = new Date();
    const utcNow = new Date(now.getTime() - now.getTimezoneOffset() * 60000);
    updatetime = utcNow.toISOString().slice(0, 19).replace("T", " ");
    return updatetime;
}

module.exports = {
    getCurrentTime
}