# PaymentRequestCreateRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jwt** | Option<**String**> | JWT for Ledger API authentication | [optional]
**seller_party_id** | Option<**String**> | Seller party ID (e.g., \"ext-user-phantom-1::1220...\") | [optional]
**seller_private_key** | Option<**String**> | Base58-encoded Ed25519 private key | [optional]
**service_cid** | **String** | Contract ID of AppService | 
**buyer_party_id** | **String** | Party ID of buyer who will fund payment | 
**amount** | **String** | Amount to lock (in CC) | 
**minimum** | **String** | Minimum amount to keep locked | 
**expires** | Option<**String**> | ISO 8601 expiry time (default 1 day from now) | [optional]
**description** | Option<**String**> |  | [optional]
**reference** | Option<**String**> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


