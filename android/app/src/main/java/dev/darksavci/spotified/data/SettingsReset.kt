package dev.darksavci.spotified.data

/**
 * The settings back as a new install has them, but for what is the listener's
 * own rather than a preference: what they blocked, and the Listen Together
 * servers and names they saved.
 */
fun Settings.resetKeepingYours(): Settings = Settings(
    blocked = blocked,
    togetherServers = togetherServers,
    togetherSelected = togetherSelected,
    togetherName = togetherName,
    togetherRoomName = togetherRoomName,
    togetherMode = togetherMode,
    togetherSharePicture = togetherSharePicture,
    togetherNotifications = togetherNotifications,
)
