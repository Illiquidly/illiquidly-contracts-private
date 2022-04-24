import { LCDClient, TxLog } from '@terra-money/terra.js';
import pLimit from 'p-limit';
import { env } from './env_helper';
import { keysToCamel } from './utils/js/keysToCamel';
import { asyncAction } from './utils/js/asyncAction';
import { countBy, isNumber, last } from 'lodash';
import Redis from 'ioredis';
import { table } from 'console';

const limitPromise = pLimit(10);

const lcdClient = new LCDClient(env.chain);

// Install MYSQL db https://www.digitalocean.com/community/tutorials/how-to-install-mysql-on-ubuntu-18-04
// CREATE DATABASE TRADES
export type Asset = {
  [asset: string]: {
    address: string
    amount?: number
    tokenId?: string
    denom?: string
  }
}
type TradeInfo = {
  acceptedInfo?: any // TODO correct this type
  assetsWithdrawn: boolean
  associatedAssets: Asset[]
  lastCounterId?: number
  additionnalInfo: {
    ownerComment: {
      comment: string
      time: string
    }
    time: string
    nftsWanted: string[]
    traderComment?: {
      comment: string
      time: string
    }
  }
  owner: string
  state: string
  whitelistedUsers: string[]
}

enum TradeState {
  Created = 'Created',
  Published = 'Published',
  Countered = 'Countered',
  Refused = 'Refused',
  Accepted = 'Accepted',
  Cancelled = 'Cancelled',
}
function stateToString(state: TradeState): string{
  return TradeState[state];
}

interface Trade {
  tradeId: number
  counterId?: number
  tradeInfo: TradeInfo
}

interface UseTrades {
  states?: TradeState[] | undefined
  owner?: string | undefined
  startAfter?: number | undefined
  limit?: number | undefined
  whitelistedUser?: string
  wantedNFT?: string
  containsToken?: string
  counterer?: string
  hasWhitelist?: boolean
}

interface UseCounterTrades {
  states?: TradeState[] | undefined
  owner?: string | undefined
  startAfter?: number | undefined
  limit?: number | undefined
  whitelistedUser?: string
  wantedNFT?: string
  containsToken?: string
}

async function getAllTrades(
  states?: TradeState[],
  filterOwner?: string,
  startAfter?: number,
  limit?: number,
  whitelistedUser?: string,
  wantedNFT?: string,
  containsToken?: string,
  counterer?: string,
  hasWhitelist?: boolean
): Promise<Trade[]> {
  const p2pContractAddress = env.contracts.p2p

  const tradeInfoResponse: any = await lcdClient.wasm.contractQuery(p2pContractAddress, {
    get_all_trades: {
      ...(isNumber(startAfter) ? { start_after: startAfter } : {}),
      ...(limit ? { limit } : {}),
      filters: {
        ...(states ? { states } : {}),
        ...(filterOwner ? { owner: filterOwner } : {}),
        ...(whitelistedUser ? { whitelisted_user: whitelistedUser } : {}),
        ...(wantedNFT ? { wanted_nft: wantedNFT } : {}),
        ...(containsToken ? { contains_token: containsToken } : {}),
        ...(counterer ? { counterer } : {}),
        ...(typeof hasWhitelist === 'boolean'
          ? { has_whitelist: hasWhitelist }
          : {}),
      },
    },
  })

  return tradeInfoResponse?.trades.map((trade: any): Trade => keysToCamel(trade))
}

async function getAllCounterTrades(
  states?: TradeState[],
  filterOwner?: string,
  startAfter?: number,
  limit?: number,
  whitelistedUser?: string,
  wantedNFT?: string,
  containsToken?: string,
  hasWhitelist?: boolean
): Promise<Trade[]> {
  const p2pContractAddress = env.contracts.p2p

  const counterTradeInfoResponse: any = await limitPromise(()=> lcdClient.wasm.contractQuery(
    p2pContractAddress,
    {
      get_all_counter_trades: {
        ...(isNumber(startAfter) ? { start_after: startAfter } : {}),
        ...(limit ? { limit } : {}),
        filters: {
          ...(states ? { states } : {}),
          ...(filterOwner ? { owner: filterOwner } : {}),
          ...(whitelistedUser ? { whitelisted_user: whitelistedUser } : {}),
          ...(wantedNFT ? { wanted_nft: wantedNFT } : {}),
          ...(containsToken ? { contains_token: containsToken } : {}),
          ...(typeof hasWhitelist === 'boolean'
            ? { has_whitelist: hasWhitelist }
            : {}),
        },
      },
    }
  ))

  return counterTradeInfoResponse?.counter_trades.map(
    (counterTrade: any): Trade => keysToCamel(counterTrade)
  )
}


const fetchAllTradesUntilEnd = async ({
    states,
    owner,
    startAfter,
    limit,
    whitelistedUser,
    wantedNFT,
    containsToken,
    counterer,
    hasWhitelist,
  }: UseTrades): Promise<Trade[]> => {
  let result: Trade[] = []
  let fetchSomeMore = true;

  while(fetchSomeMore){
    // eslint-disable-next-line @typescript-eslint/no-unused-vars

    const [error, trades]: any[] = await asyncAction(
      getAllTrades(
        states,
        owner,
        startAfter,
        limit,
        whitelistedUser,
        wantedNFT,
        containsToken,
        counterer,
        hasWhitelist
      )
    )
    let tradesResponse = trades as Trade[];
    if(error || !tradesResponse || tradesResponse.length == 0){
      fetchSomeMore = false;
    }else{
          result = [...result, ...tradesResponse.filter(x => x.tradeInfo)]
          startAfter = last(tradesResponse)?.tradeId;
    } 
  }
  return result
}

const fetchAllCounterTradesUntilEnd = async ({
  states,
  owner,
  startAfter,
  limit,
  whitelistedUser,
  wantedNFT,
  containsToken
}: UseCounterTrades): Promise<Trade[]> => {
  let result: Trade[] = []
  let fetchSomeMore = true;

  while(fetchSomeMore){
    // eslint-disable-next-line @typescript-eslint/no-unused-vars

    const [error, trades]: any[] = await asyncAction(
      getAllCounterTrades(
        states,
        owner,
        startAfter,
        limit,
        whitelistedUser,
        wantedNFT,
        containsToken
      )
    )
    let tradesResponse = trades as Trade[];
    if(error || !tradesResponse || tradesResponse.length == 0){
      fetchSomeMore = false;
    }else{
          result = [...result, ...tradesResponse.filter(x => x.tradeInfo)]
          startAfter = last(tradesResponse)?.tradeId;
    } 
  }
  return result
}


async function getCounteredTrades(myAddress: null | string = null){
  let params = {
    states: [TradeState.Countered],
    owner: myAddress ?? undefined,
  }
  return await fetchAllTradesUntilEnd(params);
}

async function getAssociatedTrades(counterTrades: Trade[]): Promise<any[]>{
  return Promise.all(counterTrades.map((counterTrade) => new Promise(async (resolve, reject) => 
      resolve([
        counterTrade,
        keysToCamel(await
          lcdClient.wasm.contractQuery(
            env.contracts.p2p,
            {
              trade_info:{
                trade_id: counterTrade.tradeId
              }
            }
          )
        )
    ]))))
}

async function getAssociatedAcceptedCounterTrades(trades: Trade[]): Promise<any[]>{
  return Promise.all(trades.map((trade) => new Promise(async (resolve, reject) => 
      resolve([
        trade,
        keysToCamel(await
          lcdClient.wasm.contractQuery(
            env.contracts.p2p,
            {
              counter_trade_info:{
                trade_id: trade.tradeId,
                counter_id: trade.tradeInfo.acceptedInfo.counterId
              }
            }
        )
      )
    ]))))
}

async function getCancelledCounterTradesAndTrade(myAddress: null | string = null): Promise<any[]>{
  let params = {
    states: [TradeState.Cancelled],
    owner: myAddress ?? undefined,
  }
  const result = await fetchAllCounterTradesUntilEnd(params);
  return await getAssociatedTrades(result);
}

async function getReviewedTrades(myAddress: null | string = null){
  let counterTrades = await getCancelledCounterTradesAndTrade(myAddress)
  return counterTrades.filter(([_, trade]: [Trade, TradeInfo]) => {
    return [TradeState.Countered, TradeState.Published].map(stateToString).includes(trade.state)
  });
}

async function getDeclinedTrades(myAddress: null | string = null){
  let counterTrades = await getCancelledCounterTradesAndTrade(myAddress)
  return counterTrades.filter(([_, trade]: [Trade, TradeInfo]) => {
    return ![TradeState.Countered, TradeState.Published].map(stateToString).includes(trade.state)
  });
}

async function getAcceptedCounterTradesToWithdraw(myAddress: null | string = null){
  let params = {
    states: [TradeState.Accepted],
    owner: myAddress ?? undefined,
  }
  let counterTrades = await fetchAllCounterTradesUntilEnd(params);
  let associatedTrades = await getAssociatedTrades(counterTrades);
  return associatedTrades.filter(([_,trade]: [Trade, any])=> !trade.assetsWithdrawn)
}

async function getAcceptedTradesToWithdraw(myAddress: null | string = null){
  let params = {
    states: [TradeState.Accepted],
    owner: myAddress ?? undefined,
  }
  let trades = await fetchAllTradesUntilEnd(params);
  let associatedTrades = await getAssociatedAcceptedCounterTrades(trades);
  return associatedTrades.filter(([_,counterTrade]: [Trade, TradeInfo])=> !counterTrade.assetsWithdrawn)
}

enum Action{
  CounteredTrade,
  ReviewedTrade,
  DeclinedTrade,
  WithdrawAcceptedTrade,
  WithdrawAcceptedCounterTrade,
  WithdrawCancelledTrade,
  WithdrawCancelledCounterTrade
}


async function initDB() {
  // We start the db
  return new Redis();
}
function readKey(userAddress: string){
  return `${userAddress}|notificationRead`;
}
function keysToObject(array: string[]){
  return array.reduce((r: any, a: string, i: number, aa: string[]) => {
      if (i & 1) {
          r[aa[i - 1]] = a;
      }
      return r;
  }, {});
}
async function updateNotification(db: Redis, userAddress: string, tradeId: number, counterId: number | string = "NULL", action: Action){
  let key = readKey(userAddress);
  await db.xadd(key, "*", "tradeId", tradeId, "counterId", counterId, "action", action);
}

async function getNotifications(db: Redis, userAddress: string): Promise<[string, string[]][] | undefined>{
  let key = readKey(userAddress);
  let results = await db.xread("BLOCK", 100, "STREAMS", key, 0);
  if(results){  
    let [_, messages] = results[0];
    return messages;
  }
}

function filterNotification(notifications: any[] | undefined, tradeId: number, counterId: number | string = "NULL"){
  return notifications?.filter((record: any[]) =>{
    let data = record[1];
    return data.tradeId == tradeId && data.counterId == counterId
  })
}

async function updateTradeDB(db: any){
  const trades = await fetchAllTradesUntilEnd({});
  return Promise.all(trades.map((trade: Trade)=>{
    asyncAction(db.set(`tradeInfo${trade.tradeId}`, trade.tradeInfo))
  }));

}

async function updateCounterTradeDB(db:any){
  const counterTrades = await fetchAllCounterTradesUntilEnd({});
  return Promise.all(counterTrades.map((counterTrade: Trade)=>{
    asyncAction(db.set(`counterTradeInfo${counterTrade.tradeId}-${counterTrade.tradeId}`, counterTrade.tradeInfo))
  }))
}

const knex = require('knex')({
  client: 'mysql2',
  connection: {
    host : '127.0.0.1',
    user : 'illiquidly',
    password : 'illiquidly',
    database : 'TRADES'
  }
});

async function addToDb({tradeId, counterId, tradeInfo}: Trade){
  return await knex('trades').insert({
    trade_id: tradeId,
    owner: tradeInfo.owner,
    time: tradeInfo.additionnalInfo.time,
    last_counter_id: tradeInfo.lastCounterId,
    owner_comment: tradeInfo.additionnalInfo.ownerComment.comment,
    owner_comment_time: tradeInfo.additionnalInfo.ownerComment.time,
    trader_comment: tradeInfo.additionnalInfo.traderComment?.comment,
    trader_comment_time: tradeInfo.additionnalInfo.traderComment?.time,
    state: tradeInfo.state,
    accepted_counter_trade_id: tradeInfo.acceptedInfo?.counterId,
    assets_withdrawn: tradeInfo.assetsWithdrawn,
  })
  .onConflict()
  .merge() // We erase if the data is already present
}

async function main() {

    //await knex.schema.dropTable("trades")
    //.catch(() =>{});
    await knex.schema.dropTable("trade_associated_assets")
    .catch(() =>{});
    await knex.schema.dropTable('whitelisted_users')
    .catch(() =>{});
    await knex.schema.dropTable('nfts_wanted')
    .catch(() =>{});
    await knex.schema.createTable('trades', (table: any) =>{
      table.integer("trade_id");
      table.integer("last_counter_id");
      table.string("owner_comment");
      table.string("owner_comment_time");
      table.string("time");
      table.string("trader_comment");
      table.string("trader_comment_time");
      table.string("owner");
      table.string("state");
      table.boolean("assets_withdrawn");
      table.integer("accepted_counter_trade_id");
      table.primary("trade_id");
    })
    .catch(()=> console.log("Trade table exists already"));

    await knex.schema.createTable('trade_associated_assets', (table: any) =>{
      table.integer("trade_id");
      table.string("asset_type");
      table.string("address");
      table.integer("amount");
      table.string("token_id");
      table.string("denom");
      table.primary("trade_id");
    }) 
    .catch(()=> console.log("Associated assets table exists already"));

    await knex.schema.createTable('whitelisted_users', (table: any) =>{
      table.integer("trade_id");
      table.string("user");
      table.primary("trade_id");
    })
    .catch(()=> console.log("Whitelist table exists already"));

    await knex.schema.createTable('nfts_wanted', (table: any) =>{
      table.integer("trade_id");
      table.string("address");
      table.primary("trade_id");
    })
    .catch(()=> console.log("NFTS wanted table exists already"));

    // We fetch all trades and feed them to our database
    await fetchAllTradesUntilEnd({}).then(async (trades) => Promise.all(trades.map((trade)=>{
      trade.tradeInfo.additionnalInfo.time = "";
      asyncAction(addToDb(trade))
    })));

    let test = await knex("trades").select("*");
    console.log(test);
    knex.destroy();
  /*
  let result;
  // As a trader : 
      // When your trade has been countered
  result = await getCounteredTrades();
  console.log(result);

  // As a counter-trader : 
      // When a trader reviews your trade (status Cancelled) and that trade is still in published/countered state.
  result = await getReviewedTrades();
  console.log(result); 
      // When a trader declines your offer
  result = await getDeclinedTrades();
  console.log(result);


  // In general when you need to withdraw funds
  result = await getAcceptedTradesToWithdraw();
  console.log(result);

  // In general when you need to withdraw funds
  result = await getAcceptedCounterTradesToWithdraw();
  console.log(result);

  

  let db = await initDB();
  db.flushall();
  let userAddress = "test";
  let tradeId = 9;
  let counterId = 2;
  await updateNotification(db, userAddress, tradeId, counterId, Action.CounteredTrade);
  await updateNotification(db, userAddress, tradeId+1, counterId, Action.CounteredTrade);
  const allNotif = await getNotifications(db, userAddress);
  const parsedNotif = allNotif?.map(([key, data]: [string, string[]])=> [key, keysToObject(data)]);
  const filteredNotif = filterNotification(parsedNotif, tradeId, counterId);
  if(filteredNotif){
    console.log(filteredNotif); 
  }

  db.quit();
  */
  /*
  let db = await initDB();
  db.flushall();
  await Promise.all([updateTradeDB(db), updateCounterTradeDB(db)]);  
  db.quit();
  */

}

main()
