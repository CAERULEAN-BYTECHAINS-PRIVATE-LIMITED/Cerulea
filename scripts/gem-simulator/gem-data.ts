/**
 * Reference data taken from live GeM bid documents and the underlying orders.
 *
 * Nothing in this file is invented where a real value exists. Sources are listed in
 * README.md; the short version:
 *
 *   - Bid number format `GEM/YYYY/B/NNNNNNN` and the Bid Details field labels come from
 *     published GeM bid documents (e.g. GEM/2025/B/6798497, GEM/2025/B/6563045,
 *     GEM/2023/B/2949270, GEM/2022/B/2558157).
 *   - EMD is bid security, conventionally around 1% of estimated value with buyers able
 *     to set it between 0.5% and 5%; ePBG runs 3%-10% of order value.
 *   - HSN codes are the real four-digit headings for each ministry's product category.
 *   - MSME thresholds are the Udyam classification revised with effect from 01.04.2025.
 *
 * The ministry list is NOT duplicated here — it is read from
 * `scripts/seed-ministries/ministries.json`, the same file `seed.ts` seeds the chain
 * from, so the simulator can never drift from what is actually on chain and can never
 * name a ministry that does not exist.
 */

import fs from 'node:fs';
import path from 'node:path';
import type {
  GemBidType,
  GemEvaluationMethod,
  GemPacketType,
  MinistryRow,
  MsmeCategory,
} from './types';

// ---------------------------------------------------------------------------------
// The 21 nodal ministries, read from the seed script's own source of truth.
// ---------------------------------------------------------------------------------

const MINISTRIES_JSON = path.join(__dirname, '..', 'seed-ministries', 'ministries.json');

let cachedMinistries: MinistryRow[] | null = null;

export function loadMinistries(): MinistryRow[] {
  if (cachedMinistries) return cachedMinistries;
  let raw: string;
  try {
    raw = fs.readFileSync(MINISTRIES_JSON, 'utf8');
  } catch (err) {
    throw new Error(
      `Could not read ${MINISTRIES_JSON}. The GeM simulator reads the ministry list from ` +
        `the seed script's own JSON so the two can never disagree. ` +
        `Underlying error: ${(err as Error).message}`,
    );
  }
  const rows = JSON.parse(raw) as MinistryRow[];
  if (rows.length !== 21) {
    throw new Error(
      `ministries.json should hold exactly 21 nodal ministries (PoC document Table 11); ` +
        `found ${rows.length}. Refusing to generate records against a modified list.`,
    );
  }
  cachedMinistries = rows;
  return rows;
}

export function ministry(ministryId: string): MinistryRow {
  const row = loadMinistries().find((m) => m.ministry_id === ministryId);
  if (!row) {
    const known = loadMinistries()
      .map((m) => m.ministry_id)
      .join(', ');
    throw new Error(
      `Unknown ministry_id "${ministryId}". The simulator never invents a ministry; ` +
        `use one of: ${known}`,
    );
  }
  return row;
}

/** The Class-I / Class-II thresholds the classification pallet will actually apply. */
export function thresholdsFor(ministryId: string): { classOneBps: number; classTwoBps: number } {
  const row = ministry(ministryId);
  const first = row.hsn_thresholds[0];
  // Mirrors `Pallet::thresholds_for_rule`: first configured entry, DPIIT default if none.
  if (!first) return { classOneBps: 5_000, classTwoBps: 2_000 };
  return { classOneBps: first.class_one_bps, classTwoBps: first.class_two_bps };
}

// ---------------------------------------------------------------------------------
// Money helpers. Everything the chain sees is paise (build contract section 1).
// ---------------------------------------------------------------------------------

export const PAISE_PER_RUPEE = 100n;
export const ONE_LAKH_RUPEES = 100_000n;
export const ONE_CRORE_RUPEES = 10_000_000n;

/** Rs 3 lakh — above this a GeM procurement must go out as a bid, not a direct purchase. */
export const GEM_BID_THRESHOLD_PAISE = 3n * ONE_LAKH_RUPEES * PAISE_PER_RUPEE;
/** Rs 10 crore — the DPIIT default certification threshold (PoC Table 10). */
export const RS_10_CRORE_PAISE = 10n * ONE_CRORE_RUPEES * PAISE_PER_RUPEE;
/** Rs 200 crore — the P5 domestic-restriction limit. */
export const RS_200_CRORE_PAISE = 200n * ONE_CRORE_RUPEES * PAISE_PER_RUPEE;

export function crorePaise(crore: number): bigint {
  // Via paise-per-crore to keep the arithmetic integral for fractional crore figures.
  return BigInt(Math.round(crore * 1_000_000_000));
}

export function lakhPaise(lakh: number): bigint {
  return BigInt(Math.round(lakh * 10_000_000));
}

export function rupeesPaise(rupees: number): bigint {
  return BigInt(Math.round(rupees * 100));
}

/** Paise formatted the way an Indian procurement officer reads it. */
export function formatPaise(paise: bigint | string): string {
  const value = BigInt(paise);
  const rupees = value / PAISE_PER_RUPEE;
  if (rupees >= ONE_CRORE_RUPEES) {
    return `Rs ${trim(Number(rupees) / 1e7)} crore`;
  }
  if (rupees >= ONE_LAKH_RUPEES) {
    return `Rs ${trim(Number(rupees) / 1e5)} lakh`;
  }
  return `Rs ${rupees.toLocaleString('en-IN')}`;
}

function trim(n: number): string {
  return n.toFixed(2).replace(/\.00$/, '').replace(/(\.\d)0$/, '$1');
}

/** Basis points as a display string. 5000 -> "50%", 1999 -> "19.99%". */
export function formatBps(bps: number): string {
  return `${trim(bps / 100)}%`;
}

// ---------------------------------------------------------------------------------
// EMD and ePBG. GeM sets bid security as a percentage of estimated bid value.
// ---------------------------------------------------------------------------------

/** Buyers may set EMD anywhere in 0.5%-5%; 1%-2% is what published bids actually use. */
export const EMD_PERCENT_CHOICES: readonly number[] = [0.5, 1, 1.5, 2, 2.5, 3];
/** ePBG runs 3%-10% of order value; 3% and 5% dominate published bids. */
export const EPBG_PERCENT_CHOICES: readonly number[] = [3, 5, 10];
/** ePBG must cover delivery + warranty + a claim period, so durations are long. */
export const EPBG_DURATION_MONTHS_CHOICES: readonly number[] = [14, 18, 24, 30, 38, 62];
/** "Bid Offer Validity (From End Date)" values seen on live bids. */
export const BID_VALIDITY_DAYS_CHOICES: readonly number[] = [30, 60, 90, 120];
export const TECHNICAL_CLARIFICATION_DAYS_CHOICES: readonly number[] = [2, 3, 5, 7];

export const BID_TYPES: readonly GemBidType[] = [
  'Standard Bid',
  'Custom Catalogue-Based Bid',
  'BOQ Bid',
  'Bid-to-RA',
];
export const PACKET_TYPES: readonly GemPacketType[] = ['Single Packet Bid', 'Two Packet Bid'];
export const EVALUATION_METHODS: readonly GemEvaluationMethod[] = [
  'Total value wise evaluation',
  'Item wise evaluation',
];

// ---------------------------------------------------------------------------------
// MSME classification, Udyam, revised with effect from 01.04.2025.
// Only Micro and Small get the GeM MSE purchase preference; Medium does not.
// ---------------------------------------------------------------------------------

export interface MsmeBand {
  category: MsmeCategory;
  maxTurnoverCrore: number;
  isMse: boolean;
}

export const MSME_BANDS: readonly MsmeBand[] = [
  { category: 'Micro', maxTurnoverCrore: 10, isMse: true },
  { category: 'Small', maxTurnoverCrore: 100, isMse: true },
  { category: 'Medium', maxTurnoverCrore: 500, isMse: false },
  { category: 'Not Registered', maxTurnoverCrore: 5_000, isMse: false },
];

// ---------------------------------------------------------------------------------
// Vendor identity. Fictional names in the PoC document's own idiom (an Indian river or
// place name plus a sector noun) — see the note on `GemVendor` for why no real company
// is ever named.
// ---------------------------------------------------------------------------------

export const PLACE_STEMS: readonly string[] = [
  'Sabarmati',
  'Chambal',
  'Godavari',
  'Narmada',
  'Kaveri',
  'Tungabhadra',
  'Mahanadi',
  'Brahmaputra',
  'Sutlej',
  'Bhima',
  'Periyar',
  'Gomti',
  'Damodar',
  'Betwa',
  'Zuari',
  'Indravati',
  'Manjira',
  'Sharavathi',
  'Wainganga',
  'Subarnarekha',
  'Pennar',
  'Kosi',
  'Sone',
  'Teesta',
  'Barakar',
  'Hemavathi',
  'Malaprabha',
  'Aravalli',
  'Nilgiri',
  'Girnar',
];

export const SECTOR_NOUNS: Record<string, readonly string[]> = {
  electronics: ['Devices', 'Systems', 'Electronics', 'Microsystems', 'Technologies'],
  telecom: ['Telecom', 'Networks', 'Communications', 'Datacom'],
  rail: ['Rail Systems', 'Rolling Stock', 'Wagon Works'],
  pharma: ['Pharmaceuticals', 'Lifesciences', 'Medtech', 'Healthcare'],
  energy: ['Solar', 'Energy', 'Renewables', 'Power Systems'],
  steel: ['Metals', 'Alloys', 'Steelworks', 'Forgings'],
  textile: ['Textile Mills', 'Weaving Mills', 'Apparel'],
  works: ['Infratech', 'Engineering Works', 'Constructions', 'Projects'],
  general: ['Industries', 'Instruments', 'Precision Products', 'Automation'],
};

export const ORG_SUFFIXES: readonly string[] = [
  'Private Limited',
  'Limited',
  'India Private Limited',
  'LLP',
];

/** GSTIN state codes with a representative city, for structurally valid identifiers. */
export interface StateCode {
  code: string;
  state: string;
  udyamCode: string;
  cities: readonly string[];
}

export const STATE_CODES: readonly StateCode[] = [
  { code: '27', state: 'Maharashtra', udyamCode: 'MH', cities: ['Pune', 'Nashik', 'Aurangabad'] },
  { code: '29', state: 'Karnataka', udyamCode: 'KR', cities: ['Bengaluru', 'Mysuru', 'Hubballi'] },
  { code: '33', state: 'Tamil Nadu', udyamCode: 'TN', cities: ['Coimbatore', 'Hosur', 'Chennai'] },
  { code: '24', state: 'Gujarat', udyamCode: 'GJ', cities: ['Vadodara', 'Rajkot', 'Sanand'] },
  { code: '09', state: 'Uttar Pradesh', udyamCode: 'UP', cities: ['Noida', 'Kanpur', 'Lucknow'] },
  { code: '07', state: 'Delhi', udyamCode: 'DL', cities: ['New Delhi'] },
  { code: '06', state: 'Haryana', udyamCode: 'HR', cities: ['Faridabad', 'Manesar', 'Panipat'] },
  { code: '23', state: 'Madhya Pradesh', udyamCode: 'MP', cities: ['Indore', 'Pithampur'] },
  { code: '08', state: 'Rajasthan', udyamCode: 'RJ', cities: ['Jaipur', 'Bhiwadi'] },
  { code: '19', state: 'West Bengal', udyamCode: 'WB', cities: ['Kolkata', 'Durgapur', 'Howrah'] },
  { code: '36', state: 'Telangana', udyamCode: 'TS', cities: ['Hyderabad', 'Medak'] },
  { code: '32', state: 'Kerala', udyamCode: 'KL', cities: ['Kochi', 'Thrissur'] },
  { code: '03', state: 'Punjab', udyamCode: 'PB', cities: ['Ludhiana', 'Mohali'] },
  { code: '21', state: 'Odisha', udyamCode: 'OD', cities: ['Rourkela', 'Bhubaneswar'] },
  { code: '10', state: 'Bihar', udyamCode: 'BR', cities: ['Patna', 'Hajipur'] },
];

// ---------------------------------------------------------------------------------
// What each ministry actually buys: real GeM item-category names, the real four-digit
// HSN heading, the unit GeM quotes it in, and a plausible unit price.
// ---------------------------------------------------------------------------------

export interface ItemCategory {
  name: string;
  hsnCode: string;
  unit: string;
  /** Indicative unit price in rupees; multiplied by quantity to size a bid. */
  unitPriceRupees: number;
  sector: keyof typeof SECTOR_NOUNS;
}

export const ITEM_CATEGORIES: Record<string, readonly ItemCategory[]> = {
  DPIIT: [
    { name: 'Split Air Conditioner', hsnCode: '8415', unit: 'Nos', unitPriceRupees: 42_000, sector: 'general' },
    { name: 'Copier Paper (A4, 75 GSM)', hsnCode: '4802', unit: 'Ream', unitPriceRupees: 260, sector: 'general' },
    { name: 'Ordinary Portland Cement (43 Grade)', hsnCode: '2523', unit: 'Bag', unitPriceRupees: 400, sector: 'works' },
    { name: 'Passenger Lift (8 Person)', hsnCode: '8428', unit: 'Nos', unitPriceRupees: 1_450_000, sector: 'works' },
  ],
  MEITY: [
    { name: 'Desktop Computer (All in One)', hsnCode: '8471', unit: 'Nos', unitPriceRupees: 58_000, sector: 'electronics' },
    { name: 'Laptop (Notebook Computer)', hsnCode: '8471', unit: 'Nos', unitPriceRupees: 72_000, sector: 'electronics' },
    { name: 'Network Video Recorder (NVR)', hsnCode: '8521', unit: 'Nos', unitPriceRupees: 34_000, sector: 'electronics' },
    { name: 'IP Camera (Fixed Bullet)', hsnCode: '8525', unit: 'Nos', unitPriceRupees: 11_500, sector: 'electronics' },
    { name: 'Currency Counting Machine', hsnCode: '8472', unit: 'Nos', unitPriceRupees: 26_000, sector: 'electronics' },
  ],
  DOT: [
    { name: 'Enterprise Router', hsnCode: '8517', unit: 'Nos', unitPriceRupees: 245_000, sector: 'telecom' },
    { name: 'Optical Fibre Cable (24F, Armoured)', hsnCode: '8544', unit: 'Km', unitPriceRupees: 38_000, sector: 'telecom' },
    { name: 'Cellular Mobile Handset (4G)', hsnCode: '8517', unit: 'Nos', unitPriceRupees: 14_500, sector: 'telecom' },
    { name: 'Managed Layer-3 Network Switch', hsnCode: '8517', unit: 'Nos', unitPriceRupees: 168_000, sector: 'telecom' },
  ],
  DHI: [
    { name: 'Industrial Boiler (IBR, 5 TPH)', hsnCode: '8402', unit: 'Nos', unitPriceRupees: 8_500_000, sector: 'general' },
    { name: 'CNC Vertical Machining Centre', hsnCode: '8457', unit: 'Nos', unitPriceRupees: 4_200_000, sector: 'general' },
    { name: 'Electric Bus (12 m, Low Floor)', hsnCode: '8702', unit: 'Nos', unitPriceRupees: 12_500_000, sector: 'general' },
  ],
  MOPNG: [
    { name: 'Seamless Line Pipe (API 5L X65)', hsnCode: '7304', unit: 'Metric Ton', unitPriceRupees: 118_000, sector: 'steel' },
    { name: 'Wellhead Christmas Tree Assembly', hsnCode: '8481', unit: 'Nos', unitPriceRupees: 6_400_000, sector: 'general' },
    { name: 'LPG Cylinder (14.2 kg)', hsnCode: '7311', unit: 'Nos', unitPriceRupees: 2_150, sector: 'steel' },
  ],
  DCPC: [
    { name: 'Sulphuric Acid (Technical Grade)', hsnCode: '2807', unit: 'Metric Ton', unitPriceRupees: 12_500, sector: 'general' },
    { name: 'Polypropylene Granules', hsnCode: '3902', unit: 'Metric Ton', unitPriceRupees: 108_000, sector: 'general' },
  ],
  MOHUA: [
    { name: 'Metro Rail Signalling System (CBTC)', hsnCode: '8530', unit: 'Lot', unitPriceRupees: 62_000_000, sector: 'works' },
    { name: 'Solid Waste Compactor Vehicle', hsnCode: '8705', unit: 'Nos', unitPriceRupees: 3_850_000, sector: 'works' },
    { name: 'LED Street Light Luminaire (90 W)', hsnCode: '9405', unit: 'Nos', unitPriceRupees: 4_900, sector: 'works' },
    { name: 'Structural Steel Fabrication for Viaduct', hsnCode: '7308', unit: 'Metric Ton', unitPriceRupees: 96_000, sector: 'works' },
  ],
  MOT: [
    { name: 'Cotton Bed Sheet (Hospital Grade)', hsnCode: '6302', unit: 'Nos', unitPriceRupees: 480, sector: 'textile' },
    { name: 'Woollen Blanket (Relief Grade)', hsnCode: '6301', unit: 'Nos', unitPriceRupees: 690, sector: 'textile' },
    { name: 'Uniform Fabric (Poly-Viscose)', hsnCode: '5515', unit: 'Metre', unitPriceRupees: 310, sector: 'textile' },
  ],
  MOS: [
    { name: 'Harbour Tug (32 T Bollard Pull)', hsnCode: '8904', unit: 'Nos', unitPriceRupees: 340_000_000, sector: 'works' },
    { name: 'Marine Navigation Buoy', hsnCode: '8907', unit: 'Nos', unitPriceRupees: 1_240_000, sector: 'general' },
  ],
  MOR: [
    { name: 'Cast Steel Bogie for BOXNHL Wagon', hsnCode: '8607', unit: 'Nos', unitPriceRupees: 610_000, sector: 'rail' },
    { name: 'Electric Point Machine (IRS S-24)', hsnCode: '8608', unit: 'Nos', unitPriceRupees: 285_000, sector: 'rail' },
    { name: 'Coach Interior Panel Set (LHB)', hsnCode: '8607', unit: 'Set', unitPriceRupees: 1_150_000, sector: 'rail' },
    { name: 'Rail Wheel (Forged, 1000 mm)', hsnCode: '8607', unit: 'Nos', unitPriceRupees: 92_000, sector: 'rail' },
  ],
  'MOD-DEFENCE': [
    { name: 'Ballistic Helmet (Level IIIA)', hsnCode: '6506', unit: 'Nos', unitPriceRupees: 21_500, sector: 'general' },
    { name: 'Field Shelter (Modular, 20 Man)', hsnCode: '6306', unit: 'Nos', unitPriceRupees: 940_000, sector: 'works' },
  ],
  DDP: [
    { name: 'Night Vision Binocular (Gen III)', hsnCode: '9013', unit: 'Nos', unitPriceRupees: 465_000, sector: 'electronics' },
    { name: 'Armoured Personnel Carrier Sub-Assembly', hsnCode: '8710', unit: 'Nos', unitPriceRupees: 8_900_000, sector: 'general' },
    { name: 'Bullet Resistant Jacket (BIS Level 4)', hsnCode: '6211', unit: 'Nos', unitPriceRupees: 38_000, sector: 'textile' },
  ],
  MOP: [
    { name: 'Distribution Transformer (100 kVA)', hsnCode: '8504', unit: 'Nos', unitPriceRupees: 385_000, sector: 'energy' },
    { name: 'ACSR Conductor (Panther)', hsnCode: '7614', unit: 'Km', unitPriceRupees: 178_000, sector: 'energy' },
    { name: 'Smart Energy Meter (Single Phase)', hsnCode: '9028', unit: 'Nos', unitPriceRupees: 2_450, sector: 'energy' },
  ],
  MNRE: [
    { name: 'Solar Photovoltaic Module (540 Wp, Mono PERC)', hsnCode: '8541', unit: 'Nos', unitPriceRupees: 12_800, sector: 'energy' },
    { name: 'Grid-Tied Solar Inverter (100 kW)', hsnCode: '8504', unit: 'Nos', unitPriceRupees: 415_000, sector: 'energy' },
    { name: 'Solar Water Pumping System (5 HP)', hsnCode: '8413', unit: 'Nos', unitPriceRupees: 268_000, sector: 'energy' },
  ],
  MOCA: [
    { name: 'X-Ray Baggage Inspection System (Dual View)', hsnCode: '9022', unit: 'Nos', unitPriceRupees: 4_850_000, sector: 'electronics' },
    { name: 'Airport Ground Power Unit (90 kVA)', hsnCode: '8502', unit: 'Nos', unitPriceRupees: 3_150_000, sector: 'energy' },
  ],
  MOSTEEL: [
    { name: 'TMT Reinforcement Bar (Fe 550D)', hsnCode: '7214', unit: 'Metric Ton', unitPriceRupees: 58_500, sector: 'steel' },
    { name: 'Hot Rolled Structural Section (ISMB)', hsnCode: '7216', unit: 'Metric Ton', unitPriceRupees: 61_200, sector: 'steel' },
    { name: 'Colour Coated Steel Sheet', hsnCode: '7210', unit: 'Metric Ton', unitPriceRupees: 79_000, sector: 'steel' },
  ],
  MOM: [
    { name: 'Underground Load Haul Dumper (6 T)', hsnCode: '8430', unit: 'Nos', unitPriceRupees: 21_500_000, sector: 'general' },
    { name: 'Mine Ventilation Fan (2000 mm)', hsnCode: '8414', unit: 'Nos', unitPriceRupees: 3_400_000, sector: 'general' },
  ],
  DOF: [
    { name: 'Di-Ammonium Phosphate (DAP)', hsnCode: '3105', unit: 'Metric Ton', unitPriceRupees: 27_000, sector: 'general' },
    { name: 'Urea (Neem Coated)', hsnCode: '3102', unit: 'Metric Ton', unitPriceRupees: 5_800, sector: 'general' },
  ],
  DST: [
    { name: 'UV-Visible Spectrophotometer', hsnCode: '9027', unit: 'Nos', unitPriceRupees: 1_180_000, sector: 'general' },
    { name: 'Laboratory Information Management Software', hsnCode: '8523', unit: 'Licence', unitPriceRupees: 165_000, sector: 'electronics' },
    { name: 'Environmental Test Chamber', hsnCode: '9031', unit: 'Nos', unitPriceRupees: 2_650_000, sector: 'general' },
  ],
  DAE: [
    { name: 'Area Radiation Monitoring System', hsnCode: '9030', unit: 'Nos', unitPriceRupees: 5_600_000, sector: 'electronics' },
    { name: 'Heavy Water Grade Process Pump', hsnCode: '8413', unit: 'Nos', unitPriceRupees: 7_900_000, sector: 'general' },
  ],
  DOP: [
    { name: 'Digital Radiography System (Fixed)', hsnCode: '9022', unit: 'Nos', unitPriceRupees: 6_950_000, sector: 'pharma' },
    { name: 'Multipara Patient Monitor', hsnCode: '9018', unit: 'Nos', unitPriceRupees: 96_000, sector: 'pharma' },
    { name: 'Syringe Infusion Pump', hsnCode: '9018', unit: 'Nos', unitPriceRupees: 47_500, sector: 'pharma' },
    { name: 'Paracetamol Tablets IP 500 mg', hsnCode: '3004', unit: 'Strip', unitPriceRupees: 14, sector: 'pharma' },
  ],
};

/**
 * "Buyer Organisation" on a GeM bid is a four-level hierarchy: Ministry/State Name,
 * Department Name, Organisation Name, Office Name. These are the real-world buying arms
 * of each nodal ministry.
 */
export interface BuyerOrganisation {
  ministryStateName: string;
  departmentName: string;
  organisationName: string;
  officeNames: readonly string[];
}

export const BUYER_ORGANISATIONS: Record<string, BuyerOrganisation> = {
  DPIIT: {
    ministryStateName: 'Ministry Of Commerce And Industry',
    departmentName: 'Department For Promotion Of Industry And Internal Trade',
    organisationName: 'Office Of The Controller General Of Patents Designs And Trade Marks',
    officeNames: ['Patent Office Delhi', 'Patent Office Mumbai'],
  },
  MEITY: {
    ministryStateName: 'Ministry Of Electronics And Information Technology',
    departmentName: 'Department Of Electronics And Information Technology',
    organisationName: 'National Informatics Centre',
    officeNames: ['Nic Headquarters New Delhi', 'Nic State Centre Bengaluru', 'Nic State Centre Pune'],
  },
  DOT: {
    ministryStateName: 'Ministry Of Communications',
    departmentName: 'Department Of Telecommunications',
    organisationName: 'Bharat Sanchar Nigam Limited',
    officeNames: ['Circle Office Maharashtra', 'Circle Office Tamil Nadu', 'Corporate Office New Delhi'],
  },
  DHI: {
    ministryStateName: 'Ministry Of Heavy Industries',
    departmentName: 'Department Of Heavy Industry',
    organisationName: 'Central Manufacturing Technology Institute',
    officeNames: ['Cmti Bengaluru'],
  },
  MOPNG: {
    ministryStateName: 'Ministry Of Petroleum And Natural Gas',
    departmentName: 'Ministry Of Petroleum And Natural Gas',
    organisationName: 'Oil And Natural Gas Corporation Limited',
    officeNames: ['Western Offshore Asset Mumbai', 'Ankleshwar Asset'],
  },
  DCPC: {
    ministryStateName: 'Ministry Of Chemicals And Fertilizers',
    departmentName: 'Department Of Chemicals And Petro Chemicals',
    organisationName: 'Central Institute Of Petrochemicals Engineering And Technology',
    officeNames: ['Cipet Chennai', 'Cipet Bhubaneswar'],
  },
  MOHUA: {
    ministryStateName: 'Ministry Of Housing And Urban Affairs',
    departmentName: 'Department Of Housing And Urban Affairs',
    organisationName: 'Delhi Metro Rail Corporation Limited',
    officeNames: ['Metro Bhawan New Delhi', 'Phase Iv Project Office'],
  },
  MOT: {
    ministryStateName: 'Ministry Of Textiles',
    departmentName: 'Ministry Of Textiles',
    organisationName: 'National Textile Corporation Limited',
    officeNames: ['Corporate Office New Delhi', 'Regional Office Coimbatore'],
  },
  MOS: {
    ministryStateName: 'Ministry Of Ports Shipping And Waterways',
    departmentName: 'Department Of Shipping',
    organisationName: 'Jawaharlal Nehru Port Authority',
    officeNames: ['Jnpa Nhava Sheva'],
  },
  MOR: {
    ministryStateName: 'Ministry Of Railways',
    departmentName: 'Railway Board',
    organisationName: 'Integral Coach Factory',
    officeNames: ['Icf Perambur', 'Rail Wheel Factory Yelahanka', 'Modern Coach Factory Raebareli'],
  },
  'MOD-DEFENCE': {
    ministryStateName: 'Ministry Of Defence',
    departmentName: 'Department Of Military Affairs',
    organisationName: 'Directorate General Of Ordnance Services',
    officeNames: ['Central Ordnance Depot Kanpur', 'Central Ordnance Depot Agra'],
  },
  DDP: {
    ministryStateName: 'Ministry Of Defence',
    departmentName: 'Department Of Defence Production',
    organisationName: 'Advanced Weapons And Equipment India Limited',
    officeNames: ['Kanpur Unit', 'Jabalpur Unit'],
  },
  MOP: {
    ministryStateName: 'Ministry Of Power',
    departmentName: 'Ministry Of Power',
    organisationName: 'Rural Electrification Corporation Limited',
    officeNames: ['Corporate Office Gurugram', 'Project Office Lucknow'],
  },
  MNRE: {
    ministryStateName: 'Ministry Of New And Renewable Energy',
    departmentName: 'Ministry Of New And Renewable Energy',
    organisationName: 'Solar Energy Corporation Of India Limited',
    officeNames: ['Seci Head Office New Delhi', 'Seci Regional Office Jaipur'],
  },
  MOCA: {
    ministryStateName: 'Ministry Of Civil Aviation',
    departmentName: 'Ministry Of Civil Aviation',
    organisationName: 'Airports Authority Of India',
    officeNames: ['Rajiv Gandhi Bhawan New Delhi', 'Regional Headquarters Kolkata'],
  },
  MOSTEEL: {
    ministryStateName: 'Ministry Of Steel',
    departmentName: 'Ministry Of Steel',
    organisationName: 'Steel Authority Of India Limited',
    officeNames: ['Bhilai Steel Plant', 'Rourkela Steel Plant', 'Bokaro Steel Plant'],
  },
  MOM: {
    ministryStateName: 'Ministry Of Mines',
    departmentName: 'Ministry Of Mines',
    organisationName: 'Hindustan Copper Limited',
    officeNames: ['Malanjkhand Copper Project', 'Indian Copper Complex Ghatsila'],
  },
  DOF: {
    ministryStateName: 'Ministry Of Chemicals And Fertilizers',
    departmentName: 'Department Of Fertilizers',
    organisationName: 'National Fertilizers Limited',
    officeNames: ['Panipat Unit', 'Vijaipur Unit'],
  },
  DST: {
    ministryStateName: 'Ministry Of Science And Technology',
    departmentName: 'Department Of Science And Technology',
    organisationName: 'Council Of Scientific And Industrial Research',
    officeNames: ['Csir National Physical Laboratory', 'Csir Central Electrochemical Research Institute'],
  },
  DAE: {
    ministryStateName: 'Department Of Atomic Energy',
    departmentName: 'Department Of Atomic Energy',
    organisationName: 'Nuclear Power Corporation Of India Limited',
    officeNames: ['Kakrapar Atomic Power Station', 'Tarapur Atomic Power Station'],
  },
  DOP: {
    ministryStateName: 'Ministry Of Chemicals And Fertilizers',
    departmentName: 'Department Of Pharmaceuticals',
    organisationName: 'Hll Lifecare Limited',
    officeNames: ['Corporate Office Thiruvananthapuram', 'Hll Procurement Cell New Delhi'],
  },
};

export function buyerFor(ministryId: string): BuyerOrganisation {
  const buyer = BUYER_ORGANISATIONS[ministryId];
  if (!buyer) {
    throw new Error(`No buyer organisation configured for ministry_id "${ministryId}".`);
  }
  return buyer;
}

export function itemCategoriesFor(ministryId: string): readonly ItemCategory[] {
  const items = ITEM_CATEGORIES[ministryId];
  if (!items || items.length === 0) {
    throw new Error(`No item categories configured for ministry_id "${ministryId}".`);
  }
  return items;
}
