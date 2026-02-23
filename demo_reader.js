const kren = require('@pawanxz/kren');

console.log("Starting KREN Node.js Reader...");
const channelName = "kren_demo_channel";

try {
    const reader = new kren.Reader(channelName);
    console.log(`Connected to channel: ${channelName}`);

    // Poll the buffer for new messages
    const interval = setInterval(() => {
        // Try reading without blocking
        const data = reader.tryRead();

        if (data) {
            console.log(`Received: ${data.toString('utf-8')}`);
        }

        // If writer closed the channel, stop reading
        if (reader.writerClosed && reader.available === 0) {
            console.log("Writer closed the channel. Exiting.");
            clearInterval(interval);
        }
    }, 100); // check every 100ms

} catch (error) {
    console.error(`Error: ${error.message}`);
}
