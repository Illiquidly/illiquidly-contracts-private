const axios = require("axios");
const pMap = (...args) =>
  import("p-map").then(({ default: pMap }) => pMap(...args));
const { LCDClient } = require("@terra-money/terra.js");
const express = require("express");
const { createClient } = require("redis");
const app = express();
const PORT = 8080;

function toKey(network, address) {
  return `${address}@${network}`;
}

const LCD_URLS = {
  "bombay-12": "https://bombay-lcd.terra.dev",
  "columbus-5": "https://lcd.terra.dev",
};

const FCD_URLS = {
  "bombay-12": "https://bombay-fcd.terra.dev",
  "columbus-5": "https://fcd.terra.dev",
};

function asyncAction(promise) {
  return Promise.resolve(promise)
    .then((data) => [null, data])
    .catch((error) => [error]);
}
function addFromWasmEvents(tx, nftsInteracted) {
  if (!tx.raw_log.includes("token_id")) {
    return;
  }
  if (tx.logs) {
    for (let log of tx.logs) {
      for (let event of log.events) {
        if (event.type === "wasm") {
          let hasNftTransferred = false;
          let contract;
          // We check the tx transfered an NFT
          for (let attribute of event.attributes) {
            if (
              attribute.value == "transfer_nft" ||
              attribute.value == "mint"
            ) {
              hasNftTransferred = true;
            }
            if (attribute.key == "contract_address") {
              contract = attribute.value;
            }
          }
          if (hasNftTransferred) {
            nftsInteracted.add(contract);
          }
        }
      }
    }
  }
}

function addFromMsg(tx, nftsInteracted) {
  for (let msg of tx.tx.value.msg) {
    if (msg.type == "wasm/MsgExecuteContract") {
      const executeEmg = msg.value.execute_msg;
      if (
        (executeEmg.transfer_nft || executeEmg.mint) &&
        !tx.raw_log.includes("failed")
      ) {
        nftsInteracted.add(msg.value.contract);
      }
    }
  }
}

function getNftsFromTxList(txData, minBlockHeight = 0) {
  var nftsInteracted = new Set();
  let minBlockHeightSeen = minBlockHeight;
  let lastTxIdSeen = 0;
  for (let tx of txData.data.txs) {
    if (tx.height > minBlockHeight) {
      // We add NFTS interacted with
      addFromWasmEvents(tx, nftsInteracted);
      addFromMsg(tx, nftsInteracted, minBlockHeight);
    }

    // We update the block and id info
    if (minBlockHeightSeen === 0 || tx.height < minBlockHeightSeen) {
      minBlockHeightSeen = tx.height;
    }
    if (lastTxIdSeen === 0 || tx.id < lastTxIdSeen) {
      lastTxIdSeen = tx.id;
    }
  }
  return [nftsInteracted, lastTxIdSeen, minBlockHeightSeen];
}

async function getNewInteractedNfts(network, address, lastBlockHeight) {
  let nftsInteracted = new Set();
  let behindLastBlockHeight = true;
  let limit = 100;
  let offset = 0;

  while (behindLastBlockHeight) {
    console.log("New fcd query");
    let txData = await axios
      .get(
        `${FCD_URLS[network]}/v1/txs?offset=${offset}&limit=${limit}&account=${address}`
      )
      .catch((error) => {
        if (error?.response?.status === 500) {
          // No more results
        } else {
          console.log(error);
        }
        return null;
      });
    if (txData === null) {
      behindLastBlockHeight = false;
    } else {
      let [newNfts, lastTxIdSeen, minTxHeightSeen] = getNftsFromTxList(
        txData,
        lastBlockHeight
      );
      if (lastBlockHeight && minTxHeightSeen <= lastBlockHeight) {
        behindLastBlockHeight = false;
      }
      offset = lastTxIdSeen;
      newNfts.forEach((nft) => nftsInteracted.add(nft));
    }
  }
  return nftsInteracted;
}

async function getBlockHeight(network) {
  const lcdClient = new LCDClient({
    URL: LCD_URLS[network],
    chainID: network,
  });

  const response = await lcdClient.tendermint.blockInfo();

  return response.block.header.height;
}

async function parseNFTSet(network, nfts, address) {
  const lcdClient = new LCDClient({
    URL: LCD_URLS[network],
    chainID: network,
  });

  return Object.assign(
    {},
    ...(await pMap(
      nfts,
      async (nft) => {
        const [, tokenId] = await asyncAction(
          lcdClient.wasm.contractQuery(nft, {
            tokens: { owner: address },
          })
        );

        if (tokenId) {
          // We try to fetch the tokenId info
          const [error, response] = await asyncAction(
            pMap(
              tokenId["tokens"],
              async (id) => {
                const [, nftInfo] = await asyncAction(
                  lcdClient.wasm.contractQuery(nft, {
                    nft_info: { token_id: id },
                  })
                );

                if (nftInfo) {
                  return {
                    tokenId: id,
                    nftInfo: nftInfo,
                  };
                }

                return {
                  tokenId: id,
                  nftInfo: {},
                };
              },
              { concurrency: 10 }
            )
          );

          if (error) {
            return tokenId["tokens"].map((tokenId) => ({
              tokenId: tokenId,
              nftInfo: {},
            }));
          }

          if (response) {
            return {
              [nft]: {
                contract: nft,
                tokens: response,
              },
            };
          }
        }
      },
      { concurrency: 10 }
    ))
  );
}

async function getNewDatabaseInfo(network, address, blockHeight = undefined) {
  return await getNewInteractedNfts(network, address, blockHeight);
}

function getDefaultApiStructure() {
  return {
    lastBlock: 0,
    interactedNfts: [],
    ownedNfts: {},
  };
}

async function updateAddress(
  db,
  network,
  address,
  currentData = undefined,
  lastBlock = undefined
) {
  let blockHeight = await getBlockHeight(network);

  // In case the address was never scanned (or never interacted with any NFT contract)
  if (!currentData) {
    currentData = getDefaultApiStructure();
  }
  if (!lastBlock) {
    lastBlock = currentData.lastBlock;
  }
  let newNfts = await getNewDatabaseInfo(network, address, lastBlock);
  if (newNfts.size) {
    let nfts = new Set(currentData.interactedNfts);
    newNfts.forEach((nft) => nfts.add(nft));

    currentData.interactedNfts = [...nfts];
    console.log("Added new nfts !");
  } else {
    console.log("No new nfts");
  }
  console.log("Checking property!");

  currentData.ownedNfts = await parseNFTSet(
    network,
    currentData.interactedNfts,
    address
  );

  currentData.lastBlock = blockHeight;

  await db.set(toKey(network, address), JSON.stringify(currentData), {
    NX: true,
  });

  return currentData;
}

// Allow any to access this API.
app.use(function (req, res, next) {
  res.header("Access-Control-Allow-Origin", "*");
  res.header(
    "Access-Control-Allow-Headers",
    "Origin, X-Requested-With, Content-Type, Accept"
  );
  next();
});

// Handle generic errors thrown by the express application.
function expressErrorHandler(err) {
  if (err.code === "EADDRINUSE")
    console.error(
      `Port ${port} is already in use. Is this program already running?`
    );
  else console.error(JSON.stringify(err, null, 2));

  console.error("Express could not start!");
  process.exit(0);
}

function validate(network, res) {
  if (!FCD_URLS[network]) {
    res.status(404).send({ status: "Network not found" });
    return false;
  } else {
    return true;
  }
}

async function main() {
  const db = createClient();

  await db.connect();

  app.get("/nfts/update-query/:network/:address", async (req, res) => {
    const address = req.params.address;
    const network = req.params.network
      .replace("mainnet", "columbus-5")
      .replace("testnet", "bombay-12");
    if (validate(network, res)) {
      const currentData = await db.get(toKey(network, address));

      console.error("current data", JSON.stringify(currentData));

      res
        .status(200)
        .send(
          await updateAddress(
            db,
            network,
            address,
            currentData ? JSON.parse(currentData) : null
          )
        );
    }
  });

  app.listen(PORT).on("error", expressErrorHandler);
  console.log("Express started on port " + PORT);
}

main();
