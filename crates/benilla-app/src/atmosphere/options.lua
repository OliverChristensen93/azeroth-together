-- Azeroth Together's graphics options: the atmosphere plugin's switches as a WoW options panel,
-- opened from the Game Menu (under Video Options), from a button in Video Options, and from the
-- main menu. Each switch is a CVar of its own (RegisterCVar), saved with the client's settings
-- like any video option; the Rust side (atmosphere::options) reads them every frame they change.
-- Raises until the in-game UI is loaded, so the caller tries again next frame.
if not (GameMenuFrame and GameMenuButtonOptions and GameMenuButtonSoundOptions and OptionsFrame) then
    error("in-game UI not up yet")
end
if AtmosOptionsFrame then
    return
end

local SWITCHES = {
    { "atmosWeather", "Weather", "Rain, snow and sandstorms that come and go in each zone." },
    { "atmosStorms", "Thunderstorms", "Lightning bolts, flashes and thunder in heavy rain, and the stormy colour." },
    { "atmosFog", "Fog and ground mist", "Low mist that pools in hollows and drifts, and a light haze in the distance." },
    { "atmosShafts", "Sun shafts", "Rays of light from the sun or moon through gaps in trees and roofs." },
    { "atmosOcclusion", "Contact shadows", "Soft shadows where things meet: under creatures, around rocks and walls. Costs the most." },
    { "atmosSharpen", "Sharpen and colour", "A light sharpen with a little more colour and contrast." },
    { "atmosWind", "Wind", "Gusts that slant the rain, harder in storms." },
    { "atmosDayCycle", "Time of day", "Each zone's own hour, moving on through the run. Off: always midday." },
    { "atmosRaindrops", "Raindrops on the camera", "Drops that bead and slide on the screen while it rains." },
}
local SLIDERS = {
    { "atmosFogStrength", "Fog thickness" },
    { "atmosShaftStrength", "Sun shaft strength" },
    { "atmosOcclusionStrength", "Contact shadow strength" },
}
local DEFAULTS = {
    atmosWeather = "1", atmosStorms = "1", atmosFog = "1", atmosShafts = "1", atmosOcclusion = "1",
    atmosSharpen = "1", atmosWind = "1", atmosDayCycle = "1", atmosRaindrops = "0",
    atmosFogStrength = "1", atmosShaftStrength = "1", atmosOcclusionStrength = "1",
}
for name, value in pairs(DEFAULTS) do
    RegisterCVar(name, value)
end

local f = CreateFrame("Frame", "AtmosOptionsFrame", nil)
f:SetFrameStrata("DIALOG")
f:SetToplevel(1)
f:SetWidth(380)
f:SetHeight(492)
f:SetPoint("CENTER", nil, "CENTER", 0, 20)
f:EnableMouse(1)
f:SetMovable(1)
f:SetBackdrop({
    bgFile = "Interface\\DialogFrame\\UI-DialogBox-Background",
    edgeFile = "Interface\\DialogFrame\\UI-DialogBox-Border",
    tile = true, tileSize = 32, edgeSize = 32,
    insets = { left = 11, right = 12, top = 12, bottom = 11 },
})
f:Hide()
tinsert(UISpecialFrames, "AtmosOptionsFrame")

local header = f:CreateTexture("AtmosOptionsFrameHeader", "ARTWORK")
header:SetTexture("Interface\\DialogFrame\\UI-DialogBox-Header")
header:SetWidth(320)
header:SetHeight(64)
header:SetPoint("TOP", f, "TOP", 0, 12)
local title = f:CreateFontString(nil, "ARTWORK", "GameFontNormal")
title:SetText("Azeroth Together Graphics")
title:SetPoint("TOP", header, "TOP", 0, -14)

local note = f:CreateFontString(nil, "ARTWORK", "GameFontHighlightSmall")
note:SetWidth(330)
note:SetJustifyH("LEFT")
note:SetText("Custom effects made for Azeroth Together, on top of the original 1.12 look. Turn them off if the game runs slowly on your PC.")
note:SetPoint("TOPLEFT", f, "TOPLEFT", 24, -34)

local checks = {}
for i = 1, table.getn(SWITCHES) do
    local s = SWITCHES[i]
    local b = CreateFrame("CheckButton", "AtmosOptionsCheck" .. i, f, "OptionsCheckButtonTemplate")
    b:SetPoint("TOPLEFT", f, "TOPLEFT", 22, -76 - (i - 1) * 26)
    getglobal(b:GetName() .. "Text"):SetText(s[2])
    b.cvar = s[1]
    b.tooltipText = s[3]
    b:SetScript("OnClick", function()
        if this:GetChecked() then
            PlaySound("igMainMenuOptionCheckBoxOn")
            SetCVar(this.cvar, "1")
        else
            PlaySound("igMainMenuOptionCheckBoxOff")
            SetCVar(this.cvar, "0")
        end
    end)
    checks[i] = b
end

local sliders = {}
for i = 1, table.getn(SLIDERS) do
    local s = SLIDERS[i]
    local sl = CreateFrame("Slider", "AtmosOptionsSlider" .. i, f, "OptionsSliderTemplate")
    sl:SetWidth(200)
    sl:SetPoint("TOPLEFT", f, "TOPLEFT", 90, -324 - (i - 1) * 40)
    sl:SetMinMaxValues(0, 2)
    sl:SetValueStep(0.1)
    sl.cvar = s[1]
    sl.label = s[2]
    sl.tooltipText = "0 is none, 1 the default, 2 double."
    getglobal(sl:GetName() .. "Low"):SetText("0")
    getglobal(sl:GetName() .. "High"):SetText("2")
    sl:SetScript("OnValueChanged", function()
        local v = string.format("%.1f", this:GetValue())
        getglobal(this:GetName() .. "Text"):SetText(this.label .. ": " .. v)
        if GetCVar(this.cvar) ~= v then
            SetCVar(this.cvar, v)
        end
    end)
    sliders[i] = sl
end

local function refresh()
    for i = 1, table.getn(checks) do
        checks[i]:SetChecked(GetCVar(checks[i].cvar) == "1")
    end
    for i = 1, table.getn(sliders) do
        local sl = sliders[i]
        local v = tonumber(GetCVar(sl.cvar)) or 1
        sl:SetValue(v)
        getglobal(sl:GetName() .. "Text"):SetText(sl.label .. ": " .. string.format("%.1f", v))
    end
end
f:SetScript("OnShow", function()
    PlaySound("igMainMenuOpen")
    refresh()
end)
f:SetScript("OnHide", function()
    PlaySound("igMainMenuClose")
end)

local function button(name, text, width, onclick)
    local b = CreateFrame("Button", name, f, "UIPanelButtonTemplate")
    b:SetWidth(width)
    b:SetHeight(22)
    b:SetText(text)
    b:SetScript("OnClick", onclick)
    return b
end
local allOff = button("AtmosOptionsAllOff", "All off (original look)", 160, function()
    for i = 1, table.getn(checks) do
        SetCVar(checks[i].cvar, "0")
    end
    refresh()
end)
allOff:SetPoint("BOTTOMLEFT", f, "BOTTOMLEFT", 18, 18)
local defaults = button("AtmosOptionsDefaults", "Defaults", 90, function()
    for name, value in pairs(DEFAULTS) do
        SetCVar(name, value)
    end
    refresh()
end)
defaults:SetPoint("LEFT", allOff, "RIGHT", 4, 0)
local okay = button("AtmosOptionsOkay", "Okay", 80, function()
    f:Hide()
end)
okay:SetPoint("BOTTOMRIGHT", f, "BOTTOMRIGHT", -18, 18)

-- The Game Menu gets an entry right under Video Options.
local gm = CreateFrame("Button", "GameMenuButtonAtmosOptions", GameMenuFrame, "GameMenuButtonTemplate")
gm:SetText("Azeroth Graphics")
gm:SetPoint("TOP", GameMenuButtonOptions, "BOTTOM", 0, -1)
GameMenuButtonSoundOptions:ClearAllPoints()
GameMenuButtonSoundOptions:SetPoint("TOP", gm, "BOTTOM", 0, -1)
GameMenuFrame:SetHeight(GameMenuFrame:GetHeight() + 21)
gm:SetScript("OnClick", function()
    PlaySound("igMainMenuOption")
    HideUIPanel(GameMenuFrame)
    AtmosOptionsFrame:Show()
end)

-- And Video Options a way across.
local vo = CreateFrame("Button", "OptionsFrameAtmosOptions", OptionsFrame, "UIPanelButtonTemplate")
vo:SetWidth(150)
vo:SetHeight(22)
vo:SetText("Azeroth Graphics...")
vo:SetPoint("BOTTOMLEFT", OptionsFrame, "BOTTOMLEFT", 16, 16)
vo:SetScript("OnClick", function()
    HideUIPanel(OptionsFrame)
    AtmosOptionsFrame:Show()
end)

function AtmosOptions_Toggle()
    if AtmosOptionsFrame:IsVisible() then
        AtmosOptionsFrame:Hide()
    else
        AtmosOptionsFrame:Show()
    end
end