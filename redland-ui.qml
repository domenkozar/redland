import Quickshell
import Quickshell.Io
import QtQuick
import QtQuick.Layouts

ShellRoot {
    component TrayMenuItem: Rectangle {
        property string text: ""
        property bool active: false
        signal clicked()
        
        Layout.fillWidth: true
        height: 25
        color: mouseArea.containsMouse ? "#404040" : "transparent"
        radius: 4
        
        Text {
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            anchors.leftMargin: 10
            text: parent.active ? parent.text : parent.text.replace("✓ ", "  ")
            color: parent.active ? "#00aaff" : "#cccccc"
            font.pixelSize: 13
        }
        
        MouseArea {
            id: mouseArea
            anchors.fill: parent
            hoverEnabled: true
            onClicked: parent.clicked()
        }
    }

    // Popup window for tray menu
    PopupWindow {
        id: trayMenu
        visible: false
        anchor.window: panel

        implicitWidth: 200
        implicitHeight: column.implicitHeight + 20
        
        color: "transparent"
        
        Rectangle {
            anchors.fill: parent
            color: "#2b2b2b"
            radius: 8
            border.color: "#404040"
            border.width: 1
            
            ColumnLayout {
                id: column
                anchors.fill: parent
                anchors.margins: 10
                spacing: 5
                
                // Mode selection items
                TrayMenuItem {
                    text: {
                        let icon = backend.automaticMode === "day" || backend.automaticMode === "sunrise" ? "☀" :
                                   backend.automaticMode === "sunset" ? "🌅" : "🌙"
                        let label = icon + "ᴬ Automatic"
                        return backend.requestedMode === "auto" ? "✓ " + label : "  " + label
                    }
                    active: backend.requestedMode === "auto"
                    onClicked: {
                        backend.setMode("auto")
                        trayMenu.visible = false
                    }
                }
                
                TrayMenuItem {
                    text: backend.requestedMode === "day" ?
                          "✓ ☀️ Day Mode" : "  ☀️ Day Mode"
                    active: backend.requestedMode === "day"
                    onClicked: {
                        backend.setMode("day")
                        trayMenu.visible = false
                    }
                }

                TrayMenuItem {
                    text: backend.requestedMode === "night" ?
                          "✓ 🌙 Night Mode" : "  🌙 Night Mode"
                    active: backend.requestedMode === "night"
                    onClicked: {
                        backend.setMode("night")
                        trayMenu.visible = false
                    }
                }

                TrayMenuItem {
                    text: backend.requestedMode === "sunset" ?
                          "✓ 🌅 Sunset Mode" : "  🌅 Sunset Mode"
                    active: backend.requestedMode === "sunset"
                    onClicked: {
                        backend.setMode("sunset")
                        trayMenu.visible = false
                    }
                }
                
                Rectangle {
                    Layout.fillWidth: true
                    height: 1
                    color: "#404040"
                }
                
                Text {
                    text: `Current: ${backend.currentTemp}K`
                    color: "#888888"
                    font.pixelSize: 12
                }
                
                Text {
                    text: `Range: ${backend.lowTemp}-${backend.highTemp}K`
                    color: "#888888"
                    font.pixelSize: 12
                }
            }
        }
    }
    
    // Status bar panel with tray icon
    PanelWindow {
        id: panel
        anchors {
            top: true
            right: true
        }

        implicitWidth: 40
        implicitHeight: 30
        
        color: "transparent"
        
        // Tray icon with mode indicator
        Rectangle {
            anchors.fill: parent
            anchors.margins: 4
            color: "#1a1a1a"
            radius: 4
            border.color: "#333"
            border.width: 1
            
            Text {
                anchors.centerIn: parent
                text: {
                    let displayMode = backend.requestedMode === "auto" ? backend.automaticMode : backend.currentMode
                    let icon = displayMode === "day" || displayMode === "sunrise" ? "☀" :
                               displayMode === "sunset" ? "🌅" : "🌙"
                    return backend.requestedMode === "auto" ? icon + "ᴬ" : icon
                }
                color: {
                    let displayMode = backend.requestedMode === "auto" ? backend.automaticMode : backend.currentMode
                    return displayMode === "day" || displayMode === "sunrise" ? "#ffaa00" :
                           displayMode === "sunset" ? "#ff6600" : "#6060ff"
                }
                font.pixelSize: 16
                font.bold: true
            }
            
            MouseArea {
                anchors.fill: parent
                onClicked: {
                    trayMenu.visible = !trayMenu.visible
                }
            }
        }
    }
    
    // Daemon process
    Process {
        id: daemonProcess
        command: ["/home/domen/dev/redland/target/debug/redland", "--socket", "/tmp/redland.sock"]
        running: true

        stdout: SplitParser {
            splitMarker: "\n"
            onRead: data => console.log("Daemon:", data)
        }

        stderr: SplitParser {
            splitMarker: "\n"
            onRead: data => console.log("Daemon:", data)
        }
    }

    // Backend component for IPC with Rust daemon
    Item {
        id: backend

        property string requestedMode: "auto"
        property string currentMode: "day"
        property string automaticMode: "day"
        property int currentTemp: 5500
        property int lowTemp: 4000
        property int highTemp: 6500
        property var location: null
        property var sunTimes: null

        // Socket communication
        Socket {
            id: socket
            path: "/tmp/redland.sock"

            parser: SplitParser {
                splitMarker: "\n"

                onRead: data => {
                    console.log("Received from daemon:", data)
                    try {
                        const response = JSON.parse(data)
                        if (response.type === "status") {
                            backend.requestedMode = response.requested_mode
                            backend.currentMode = response.current_mode
                            backend.automaticMode = response.automatic_mode
                            backend.currentTemp = response.current_temp
                            backend.lowTemp = response.low_temp
                            backend.highTemp = response.high_temp
                            backend.location = response.location
                            backend.sunTimes = response.sun_times
                            console.log("Updated mode to:", backend.requestedMode, "current:", backend.currentMode, "auto:", backend.automaticMode)
                        }
                    } catch (e) {
                        console.error("Failed to parse response:", e)
                    }
                }
            }

            onError: error => {
                console.error("Socket error:", error)
            }
        }

        function sendCommand(cmd) {
            const cmdJson = JSON.stringify(cmd)
            console.log("Sending command:", cmdJson)
            socket.write(cmdJson + "\n")
            socket.flush()
        }
        
        function getStatus() {
            sendCommand({"type": "get_status"})
        }
        
        function setMode(newMode) {
            sendCommand({"type": "set_mode", "mode": newMode})
        }
        
        function setTemperature(low, high) {
            sendCommand({"type": "set_temperature", "low": low, "high": high})
        }
        
        Component.onCompleted: {
            // Wait for daemon to start, then get initial status
            Qt.callLater(() => {
                statusTimer.start()
            })
        }
        
        Timer {
            id: statusTimer
            interval: 1000
            repeat: true
            onTriggered: {
                if (!socket.connected) {
                    socket.connected = true
                }
                backend.getStatus()
            }
        }
    }
}
