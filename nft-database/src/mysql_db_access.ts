import knex, { Knex } from 'knex';

let knexDB: Knex;

async function initNFTDB() {
  knexDB = knex({
    client: 'mysql2',
    connection: {
      host: '127.0.0.1',
      user: 'illiquidly',
      password: 'illiquidly',
      database: 'ILLIQUIDLY'
    }
  });
  console.log("db illiquidly")
  await flushDB();
  await createNFTInfoDB();
}

interface TokenInfo{
  network: string, 
  nftAddress: string,
  tokenId: string,
  nftInfo: any
}


async function quitNFTDB() {
  knexDB.destroy();
}

async function createNFTInfoDB(){
  await knexDB.schema
    .createTable('nft_info', (table: any) => {
      table.increments('id').primary();
      table.string("network")
      table.string("nft_address")
      table.string("name")
      table.string("symbol")
      table.unique(["network","nft_address"])
    })
    .catch(() => console.log('NFT Info table exists already'));
}

async function flushDB() {
  await knexDB.schema.dropTable('token_info').catch(() => {});
  await knexDB.schema.dropTable('nft_info').catch(() => {});
}

async function addNftInfo(nftInfo: any[]) {
  return await knexDB('nft_info')
    .insert(
      nftInfo.map((nft) => ({
        network: nft.network,
        nft_address: nft.nftAddress,
        name: nft.name,
        symbol: nft.symbol
      }))
    )
    .onConflict()
    .merge(); // We erase if the data is already present
}

async function getNftInfo(network: string, nft_address: string){
  return (await knexDB("nft_info").select("*")
    .where("network", network)
    .where("nft_address", nft_address)
    
    ).map((info)=>({
      nftAddress: info.nft_address,
      name: info.name,
      symbol: info.symbol
    }))
}

async function getAllNftInfo(network: string, nft_name: string){
  return (await knexDB("nft_info").select("*")
    .where("network", network)
    ).map((info)=>({
      nftAddress: info.nft_address,
      name: info.name,
      symbol: info.symbol
    }))
}

async function getNftInfoByName(network: string, nft_name: string){
  return (await knexDB("nft_info").select("*")
    .where("network", network)
    .where("name", nft_name)
    ).map((info)=>({
      nftAddress: info.nft_address,
      name: info.name,
      symbol: info.symbol
    }))
}




export {
  flushDB,
  initNFTDB,
  quitNFTDB,
  createNFTInfoDB,
  addNftInfo,
  getNftInfo,
  getAllNftInfo,
  getNftInfoByName,
};
